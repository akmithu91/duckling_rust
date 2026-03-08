{-# LANGUAGE ForeignFunctionInterface #-}
{-# LANGUAGE OverloadedStrings #-}
{-# LANGUAGE ScopedTypeVariables #-}

module DucklingFFI where

import Foreign.C.String (CString, newCString, peekCString)
import qualified Data.Text as T
import qualified Data.ByteString.Lazy.Char8 as BL
import Data.Aeson (encode)
import Duckling.Api (parse)
import Duckling.Core (makeLocale, Lang(EN), Region(US), Dimension(..), fromZonedTime)
import Duckling.Dimensions.Types (Seal(..))
import Duckling.Resolve (Context(..), Options(..))
import qualified Data.Time as Time
import qualified Data.Time.Zones as TZ
import Control.Exception (catch, SomeException)
import Foreign.Marshal.Alloc (free)
import Data.HashSet (HashSet)
import qualified Data.HashSet as HashSet
import Data.List (intercalate)
import Data.Char (toLower)

-- | Map a dimension name string to its corresponding Seal Dimension value.
lookupDimension :: String -> Maybe (Seal Dimension)
lookupDimension s = case map toLower s of
    "amountofmoney" -> Just (Seal AmountOfMoney)
    "creditcardnumber" -> Just (Seal CreditCardNumber)
    "distance" -> Just (Seal Distance)
    "duration" -> Just (Seal Duration)
    "email" -> Just (Seal Email)
    "numeral" -> Just (Seal Numeral)
    "ordinal" -> Just (Seal Ordinal)
    "phonenumber" -> Just (Seal PhoneNumber)
    "quantity" -> Just (Seal Quantity)
    "temperature" -> Just (Seal Temperature)
    "time" -> Just (Seal Time)
    "url" -> Just (Seal Url)
    "volume" -> Just (Seal Volume)
    _ -> Nothing

-- | All supported dimensions (used when no filter is specified).
allDimensions :: [Seal Dimension]
allDimensions =
    [ Seal AmountOfMoney
    , Seal CreditCardNumber
    , Seal Distance
    , Seal Duration
    , Seal Email
    , Seal Numeral
    , Seal Ordinal
    , Seal PhoneNumber
    , Seal Quantity
    , Seal Temperature
    , Seal Time
    , Seal Url
    , Seal Volume
    ]

-- | Parse a comma-separated dimension string into a list of Seal Dimension.
--   An empty string returns all dimensions.
parseDimensions :: String -> [Seal Dimension]
parseDimensions "" = allDimensions
parseDimensions s =
    let names = map (filter (/= ' ')) (splitOn ',' s)
        dims  = concatMap (\n -> maybe [] (:[]) (lookupDimension n)) names
    in if null dims then allDimensions else dims
  where
    splitOn :: Char -> String -> [String]
    splitOn _ [] = []
    splitOn delim str =
        let (before, rest) = break (== delim) str
        in before : case rest of
            [] -> []
            (_:xs) -> splitOn delim xs

-- | Core parsing function that accepts timezone name and dimension filter.
parseEntities :: String -> String -> String -> IO String
parseEntities inputText tzName dimsStr = do
    let textToParse = T.pack inputText

    -- 1. Resolve the timezone. Fall back to system local time on failure.
    now <- Time.getCurrentTime
    zonedTime <- if null tzName
        then Time.getZonedTime
        else do
            result <- catch
                (do tz <- TZ.loadTZFromDB tzName
                    let localT = TZ.utcToLocalTimeTZ tz now
                        tzOffset = TZ.timeZoneForUTCTime tz now
                    return (Time.ZonedTime localT tzOffset))
                (\(_ :: SomeException) -> Time.getZonedTime)
            return result

    -- 2. Construct the Context expected by Duckling
    let context = Context
            { referenceTime = fromZonedTime zonedTime
            , locale = makeLocale EN (Just US)
            }

    -- 3. Set the parsing options
    let options = Options { withLatent = False }

    -- 4. Resolve which dimensions to extract
    let dimensions = parseDimensions dimsStr

    -- 5. Execute the Duckling parse function
    let entities = parse textToParse context options dimensions

    let jsonString = BL.unpack (encode entities)
    return jsonString

-- | FFI bridge: accepts input text, timezone, and comma-separated dimensions.
c_duckling_parse :: CString -> CString -> CString -> IO CString
c_duckling_parse c_input c_tz c_dims = catch process handler
  where
    handler :: SomeException -> IO CString
    handler _ = newCString "[]"
    process = do
        haskellString <- peekCString c_input
        tzString      <- peekCString c_tz
        dimsString    <- peekCString c_dims
        jsonOutput    <- parseEntities haskellString tzString dimsString
        newCString jsonOutput

-- Define the memory deallocation wrapper
c_duckling_free_string :: CString -> IO ()
c_duckling_free_string ptr = free ptr

-- Explicitly export the functions to the C ABI
foreign export ccall "duckling_parse" c_duckling_parse :: CString -> CString -> CString -> IO CString
foreign export ccall "duckling_free_string" c_duckling_free_string :: CString -> IO ()