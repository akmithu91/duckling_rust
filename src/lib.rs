use serde::Deserialize;
use serde_json::Value;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;

/// Global lock that serialises **all** calls into the Haskell RTS.
/// The GHC runtime scheduler is not re-entrant, so concurrent FFI calls
/// from different threads will crash with "schedule: re-entered unsafely".
static HASKELL_LOCK: Mutex<()> = Mutex::new(());

/// The dimensions that Duckling can extract.
/// Pass a slice of these to `Duckling::parse` to filter results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    AmountOfMoney,
    CreditCardNumber,
    Distance,
    Duration,
    Email,
    Numeral,
    Ordinal,
    PhoneNumber,
    Quantity,
    Temperature,
    Time,
    Url,
    Volume,
}

impl Dimension {
    /// Returns the string identifier sent to the Haskell FFI layer.
    fn as_str(&self) -> &'static str {
        match self {
            Dimension::AmountOfMoney => "AmountOfMoney",
            Dimension::CreditCardNumber => "CreditCardNumber",
            Dimension::Distance => "Distance",
            Dimension::Duration => "Duration",
            Dimension::Email => "Email",
            Dimension::Numeral => "Numeral",
            Dimension::Ordinal => "Ordinal",
            Dimension::PhoneNumber => "PhoneNumber",
            Dimension::Quantity => "Quantity",
            Dimension::Temperature => "Temperature",
            Dimension::Time => "Time",
            Dimension::Url => "Url",
            Dimension::Volume => "Volume",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DucklingEntity {
    pub dim: String,
    pub body: String,
    pub start: usize,
    pub end: usize,
    pub value: Value,
}

extern "C" {
    fn duckling_parse(
        input: *const c_char,
        timezone: *const c_char,
        dimensions: *const c_char,
    ) -> *mut c_char;
    fn duckling_free_string(ptr: *mut c_char);
}

/// A configured Duckling parser instance.
///
/// # Example
/// ```no_run
/// use duckling_rust::{Duckling, Dimension};
///
/// let duckling = Duckling::new("America/Los_Angeles");
///
/// // Parse only time and money dimensions
/// let entities = duckling.parse(
///     "Lunch tomorrow at noon costs $15",
///     &[Dimension::Time, Dimension::AmountOfMoney],
/// ).unwrap();
///
/// for entity in &entities {
///     println!("{}: {}", entity.dim, entity.body);
/// }
///
/// // Parse all dimensions by passing an empty slice
/// let all = duckling.parse("Send 5 emails", &[]).unwrap();
/// ```
pub struct Duckling {
    timezone: CString,
}

impl Duckling {
    /// Create a new Duckling instance pinned to the given IANA timezone
    /// (e.g. `"America/New_York"`, `"Europe/London"`, `"UTC"`).
    ///
    /// An empty string will fall back to the system's local timezone.
    pub fn new(timezone: &str) -> Self {
        let tz = CString::new(timezone).expect("timezone must not contain null bytes");
        Duckling { timezone: tz }
    }

    /// Parse `input_text`, extracting only the specified `dimensions`.
    ///
    /// Pass an empty slice (`&[]`) to extract **all** dimensions.
    pub fn parse(
        &self,
        input_text: &str,
        dimensions: &[Dimension],
    ) -> Result<Vec<DucklingEntity>, String> {
        let c_input = CString::new(input_text)
            .map_err(|_| "Input text must not contain null bytes".to_string())?;

        let dims_csv: String = dimensions
            .iter()
            .map(|d| d.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let c_dims = CString::new(dims_csv)
            .map_err(|_| "Failed to build dimensions string".to_string())?;

        // Hold the lock for the entire FFI round-trip so the Haskell RTS
        // is never entered from more than one OS thread at a time.
        let _guard = HASKELL_LOCK
            .lock()
            .map_err(|e| format!("Haskell FFI lock poisoned: {e}"))?;

        let raw_ptr: *mut c_char = unsafe {
            duckling_parse(c_input.as_ptr(), self.timezone.as_ptr(), c_dims.as_ptr())
        };

        if raw_ptr.is_null() {
            return Err("Null pointer returned from Haskell".to_string());
        }

        let json_str = unsafe { CStr::from_ptr(raw_ptr).to_str().unwrap_or("") };
        let result = serde_json::from_str(json_str);

        unsafe {
            duckling_free_string(raw_ptr);
        }

        // _guard drops here, releasing the lock

        result.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// A single passage engineered to contain at least one instance of every
    /// Duckling dimension:
    ///
    ///   AmountOfMoney    → "$250"
    ///   CreditCardNumber → "4111111111111111"
    ///   Distance         → "5 miles"
    ///   Duration         → "2 hours"
    ///   Email            → "alice@example.com"
    ///   Numeral          → "42"
    ///   Ordinal          → "3rd" (in "the 3rd time", non-temporal context)
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
    /// greedily absorbs ordinal tokens (e.g. "3rd" → date). It is tested
    /// separately in `test_ordinal_dimension_filtered`.
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

        // Parse with empty slice → all dimensions
        let entities = duckling
            .parse(TEST_PASSAGE, &[])
            .expect("parse should succeed");

        assert!(!entities.is_empty(), "should extract at least one entity");

        let found_dims: HashSet<&str> = entities.iter().map(|e| e.dim.as_str()).collect();

        let mut missing = Vec::new();
        for dim in ALL_DIM_NAMES {
            if !found_dims.contains(dim) {
                missing.push(*dim);
            }
        }

        assert!(
            missing.is_empty(),
            "The following dimensions were NOT extracted: {:?}\n\
             Found dimensions: {:?}\n\
             Entities: {:#?}",
            missing,
            found_dims,
            entities,
        );
    }

    /// When requested in isolation (without Time competing), Duckling
    /// correctly produces ordinal entities.
    #[test]
    fn test_ordinal_dimension_filtered() {
        let duckling = Duckling::new("America/New_York");

        let entities = duckling
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

    #[test]
    fn test_filtered_dimensions() {
        let duckling = Duckling::new("America/New_York");

        // Request only Time and AmountOfMoney
        let entities = duckling
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
        let entities = duckling
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
            // The body should match the substring at [start..end]
            assert_eq!(
                &input[entity.start..entity.end],
                entity.body,
                "body should match input slice for entity: {:?}",
                entity,
            );
        }
    }
}
