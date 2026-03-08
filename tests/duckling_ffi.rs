use duckling_rust::{Duckling, Dimension, DucklingEntity};
use std::collections::HashSet;

/// A single passage engineered to contain at least one trigger for every
/// Duckling dimension:
///
///   AmountOfMoney    → "$250"
///   CreditCardNumber → "4111111111111111"
///   Distance         → "5 miles"
///   Duration         → "2 hours"
///   Email            → "alice@example.com"
///   Numeral          → "42"
///   Ordinal          → tested separately (Time absorbs ordinals)
///   PhoneNumber      → "(415) 555-1234"
///   Quantity         → "6 pounds of flour"
///   Temperature      → "72°F"
///   Time             → "tomorrow at 3pm"
///   Url              → "https://example.com"
///   Volume           → "2 gallons"
const TEST_PASSAGE: &str = "\
    Tomorrow at 3pm, I'll drive 5 miles to the store \
    and spend about 2 hours picking up 6 pounds of flour and 2 gallons of milk. \
    The total will be $250, paid with card 4111111111111111. \
    The weather is 72°F outside. For questions, call (415) 555-1234, \
    email alice@example.com, or visit https://example.com. \
    Oh and I need exactly 42 widgets.";

/// Dimension names Duckling reliably produces when parsing all dimensions
/// at once. Ordinal is excluded here because Duckling's Time dimension
/// greedily absorbs ordinal tokens. It is tested separately.
const ALL_DIM_NAMES: &[&str] = &[
    "amount-of-money",
    "credit-card-number",
    "distance",
    "duration",
    "email",
    "number",
    "phone-number",
    "quantity",
    "temperature",
    "time",
    "url",
    "volume",
];

#[test]
fn test_all_dimensions_extracted() {
    let duckling = Duckling::new("America/New_York");

    // Empty slice → parse all dimensions
    let entities: Vec<DucklingEntity> = duckling
        .parse(TEST_PASSAGE, &[])
        .expect("parse should succeed");

    assert!(!entities.is_empty(), "should extract at least one entity");

    let found_dims: HashSet<&str> = entities.iter().map(|e| e.dim.as_str()).collect();

    let missing: Vec<&&str> = ALL_DIM_NAMES
        .iter()
        .filter(|d| !found_dims.contains(**d))
        .collect();

    assert!(
        missing.is_empty(),
        "The following dimensions were NOT extracted: {:?}\nFound dimensions: {:?}\nEntities: {:#?}",
        missing,
        found_dims,
        entities,
    );
}

#[test]
fn test_filtered_dimensions() {
    let duckling = Duckling::new("America/New_York");

    let entities: Vec<DucklingEntity> = duckling
        .parse(
            "Meet me tomorrow at 3pm, it costs $50",
            &[Dimension::Time, Dimension::AmountOfMoney],
        )
        .expect("parse should succeed");

    let found_dims: HashSet<&str> = entities.iter().map(|e| e.dim.as_str()).collect();

    assert!(
        found_dims.contains("time"),
        "should find time dimension, got: {:?}",
        found_dims,
    );
    assert!(
        found_dims.contains("amount-of-money"),
        "should find amount-of-money dimension, got: {:?}",
        found_dims,
    );
}

#[test]
fn test_entity_offsets_are_valid() {
    let duckling = Duckling::new("UTC");

    let input = "I need $100 by tomorrow";
    let entities: Vec<DucklingEntity> = duckling
        .parse(input, &[])
        .expect("parse should succeed");

    for entity in &entities {
        assert!(
            entity.start <= entity.end,
            "start ({}) must be <= end ({}) for entity: {:?}",
            entity.start,
            entity.end,
            entity,
        );
        assert!(
            entity.end <= input.len(),
            "end ({}) must be <= input length ({}) for entity: {:?}",
            entity.end,
            input.len(),
            entity,
        );
        assert_eq!(
            &input[entity.start..entity.end],
            entity.body,
            "body should match input slice for entity: {:?}",
            entity,
        );
    }
}

#[test]
fn test_ordinal_dimension_filtered() {
    let duckling = Duckling::new("America/New_York");

    let entities: Vec<DucklingEntity> = duckling
        .parse(
            "I finished 3rd in the race and she was 1st",
            &[Dimension::Ordinal],
        )
        .expect("parse should succeed");

    let ordinals: Vec<&DucklingEntity> = entities
        .iter()
        .filter(|e| e.dim == "ordinal")
        .collect();

    assert!(
        !ordinals.is_empty(),
        "should extract at least one ordinal, got: {:#?}",
        entities,
    );
}