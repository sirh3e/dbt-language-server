#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

#[doc = "`AcceptedValuesTest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct AcceptedValuesTest {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub accepted_values: ::std::option::Option<AcceptedValuesTestAcceptedValues>,
}
impl AcceptedValuesTest {
    pub fn builder() -> builder::AcceptedValuesTest {
        Default::default()
    }
}
#[doc = "`AcceptedValuesTestAcceptedValues`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AcceptedValuesTestAcceptedValues {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<TestConfigs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quote: ::std::option::Option<bool>,
    pub values: ::std::vec::Vec<::std::string::String>,
    #[serde(
        rename = "where",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub where_: ::std::option::Option<::std::string::String>,
}
impl AcceptedValuesTestAcceptedValues {
    pub fn builder() -> builder::AcceptedValuesTestAcceptedValues {
        Default::default()
    }
}
#[doc = "`AggregationTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct AggregationTypeParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub percentile: ::std::option::Option<f64>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub use_approximate_percentile: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub use_discrete_percentile: ::std::option::Option<bool>,
}
impl AggregationTypeParams {
    pub fn builder() -> builder::AggregationTypeParams {
        Default::default()
    }
}
#[doc = "`ArrayOfStrings`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ArrayOfStrings(pub ::std::vec::Vec<::std::string::String>);
impl ::std::ops::Deref for ArrayOfStrings {
    type Target = ::std::vec::Vec<::std::string::String>;
    fn deref(&self) -> &::std::vec::Vec<::std::string::String> {
        &self.0
    }
}
impl ::std::convert::From<ArrayOfStrings> for ::std::vec::Vec<::std::string::String> {
    fn from(value: ArrayOfStrings) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<::std::string::String>> for ArrayOfStrings {
    fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
        Self(value)
    }
}
#[doc = "`AuthorizedViewsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct AuthorizedViewsItem {
    pub database: ::std::string::String,
    pub project: ::std::string::String,
}
impl AuthorizedViewsItem {
    pub fn builder() -> builder::AuthorizedViewsItem {
        Default::default()
    }
}
#[doc = "`BooleanOrJinjaString`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum BooleanOrJinjaString {
    Variant0(BooleanOrJinjaStringVariant0),
    Variant1(bool),
}
impl ::std::fmt::Display for BooleanOrJinjaString {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Variant0(x) => x.fmt(f),
            Self::Variant1(x) => x.fmt(f),
        }
    }
}
impl ::std::str::FromStr for BooleanOrJinjaString {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if let Ok(v) = value.parse() {
            Ok(Self::Variant0(v))
        } else if let Ok(v) = value.parse() {
            Ok(Self::Variant1(v))
        } else {
            Err("string conversion failed for all variants".into())
        }
    }
}
impl ::std::convert::TryFrom<&str> for BooleanOrJinjaString {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BooleanOrJinjaString {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::From<BooleanOrJinjaStringVariant0> for BooleanOrJinjaString {
    fn from(value: BooleanOrJinjaStringVariant0) -> Self {
        Self::Variant0(value)
    }
}
impl ::std::convert::From<bool> for BooleanOrJinjaString {
    fn from(value: bool) -> Self {
        Self::Variant1(value)
    }
}
#[doc = "`BooleanOrJinjaStringVariant0`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct BooleanOrJinjaStringVariant0(::std::string::String);
impl ::std::ops::Deref for BooleanOrJinjaStringVariant0 {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<BooleanOrJinjaStringVariant0> for ::std::string::String {
    fn from(value: BooleanOrJinjaStringVariant0) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for BooleanOrJinjaStringVariant0 {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\{\\{.*\\}\\}").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"\\{\\{.*\\}\\}\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for BooleanOrJinjaStringVariant0 {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BooleanOrJinjaStringVariant0 {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for BooleanOrJinjaStringVariant0 {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`ColumnProperties`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ColumnProperties {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub constraints: ::std::option::Option<Constraints>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub data_tests: ::std::vec::Vec<DataTests>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub data_type: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub granularity: ::std::option::Option<ColumnPropertiesGranularity>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    pub name: ::std::string::String,
    #[doc = "Configurations, specific to BigQuery adapter, used to set policy tags on specific columns, enabling column-level security. Only relevant when `persist_docs.columns` is true."]
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub policy_tags: ::std::vec::Vec<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quote: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub tests: ::std::vec::Vec<DataTests>,
}
impl ColumnProperties {
    pub fn builder() -> builder::ColumnProperties {
        Default::default()
    }
}
#[doc = "`ColumnPropertiesGranularity`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ColumnPropertiesGranularity {
    #[serde(rename = "day")]
    Day,
    #[serde(rename = "hour")]
    Hour,
    #[serde(rename = "microsecond")]
    Microsecond,
    #[serde(rename = "millisecond")]
    Millisecond,
    #[serde(rename = "minute")]
    Minute,
    #[serde(rename = "month")]
    Month,
    #[serde(rename = "nanosecond")]
    Nanosecond,
    #[serde(rename = "quarter")]
    Quarter,
    #[serde(rename = "second")]
    Second,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "year")]
    Year,
}
impl ::std::fmt::Display for ColumnPropertiesGranularity {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Day => f.write_str("day"),
            Self::Hour => f.write_str("hour"),
            Self::Microsecond => f.write_str("microsecond"),
            Self::Millisecond => f.write_str("millisecond"),
            Self::Minute => f.write_str("minute"),
            Self::Month => f.write_str("month"),
            Self::Nanosecond => f.write_str("nanosecond"),
            Self::Quarter => f.write_str("quarter"),
            Self::Second => f.write_str("second"),
            Self::Week => f.write_str("week"),
            Self::Year => f.write_str("year"),
        }
    }
}
impl ::std::str::FromStr for ColumnPropertiesGranularity {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "day" => Ok(Self::Day),
            "hour" => Ok(Self::Hour),
            "microsecond" => Ok(Self::Microsecond),
            "millisecond" => Ok(Self::Millisecond),
            "minute" => Ok(Self::Minute),
            "month" => Ok(Self::Month),
            "nanosecond" => Ok(Self::Nanosecond),
            "quarter" => Ok(Self::Quarter),
            "second" => Ok(Self::Second),
            "week" => Ok(Self::Week),
            "year" => Ok(Self::Year),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ColumnPropertiesGranularity {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ColumnPropertiesGranularity {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Constraints`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Constraints(pub ::std::vec::Vec<ConstraintsItem>);
impl ::std::ops::Deref for Constraints {
    type Target = ::std::vec::Vec<ConstraintsItem>;
    fn deref(&self) -> &::std::vec::Vec<ConstraintsItem> {
        &self.0
    }
}
impl ::std::convert::From<Constraints> for ::std::vec::Vec<ConstraintsItem> {
    fn from(value: Constraints) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<ConstraintsItem>> for Constraints {
    fn from(value: ::std::vec::Vec<ConstraintsItem>) -> Self {
        Self(value)
    }
}
#[doc = "`ConstraintsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConstraintsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub columns: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub expression: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub warn_unenforced: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub warn_unsupported: ::std::option::Option<BooleanOrJinjaString>,
}
impl ConstraintsItem {
    pub fn builder() -> builder::ConstraintsItem {
        Default::default()
    }
}
#[doc = "`ConversionMetricTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct ConversionMetricTypeParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub conversion_type_params:
        ::std::option::Option<ConversionMetricTypeParamsConversionTypeParams>,
}
impl ConversionMetricTypeParams {
    pub fn builder() -> builder::ConversionMetricTypeParams {
        Default::default()
    }
}
#[doc = "`ConversionMetricTypeParamsConversionTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ConversionMetricTypeParamsConversionTypeParams {
    pub base_measure: MetricInputMeasure,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub calculation:
        ::std::option::Option<ConversionMetricTypeParamsConversionTypeParamsCalculation>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub constant_properties:
        ::std::vec::Vec<ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem>,
    pub conversion_measure: MetricInputMeasure,
    #[doc = "The entity to calculate over"]
    pub entity: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub window: ::std::option::Option<::std::string::String>,
}
impl ConversionMetricTypeParamsConversionTypeParams {
    pub fn builder() -> builder::ConversionMetricTypeParamsConversionTypeParams {
        Default::default()
    }
}
#[doc = "`ConversionMetricTypeParamsConversionTypeParamsCalculation`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ConversionMetricTypeParamsConversionTypeParamsCalculation {
    #[serde(rename = "conversion_rate")]
    ConversionRate,
    #[serde(rename = "conversions")]
    Conversions,
}
impl ::std::fmt::Display for ConversionMetricTypeParamsConversionTypeParamsCalculation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ConversionRate => f.write_str("conversion_rate"),
            Self::Conversions => f.write_str("conversions"),
        }
    }
}
impl ::std::str::FromStr for ConversionMetricTypeParamsConversionTypeParamsCalculation {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "conversion_rate" => Ok(Self::ConversionRate),
            "conversions" => Ok(Self::Conversions),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ConversionMetricTypeParamsConversionTypeParamsCalculation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for ConversionMetricTypeParamsConversionTypeParamsCalculation
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem {
    #[doc = "DIMENSION or ENTITY"]
    pub base_property: ::std::string::String,
    #[doc = "DIMENSION or ENTITY"]
    pub conversion_property: ::std::string::String,
}
impl ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem {
    pub fn builder() -> builder::ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem
    {
        Default::default()
    }
}
#[doc = "`CumulativeMetricTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct CumulativeMetricTypeParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub cumulative_type_params:
        ::std::option::Option<CumulativeMetricTypeParamsCumulativeTypeParams>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grain_to_date: ::std::option::Option<CumulativeMetricTypeParamsGrainToDate>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub measure: ::std::option::Option<MetricInputMeasure>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub window: ::std::option::Option<::std::string::String>,
}
impl CumulativeMetricTypeParams {
    pub fn builder() -> builder::CumulativeMetricTypeParams {
        Default::default()
    }
}
#[doc = "`CumulativeMetricTypeParamsCumulativeTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct CumulativeMetricTypeParamsCumulativeTypeParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grain_to_date: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub period_agg: ::std::option::Option<CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub window: ::std::option::Option<::std::string::String>,
}
impl CumulativeMetricTypeParamsCumulativeTypeParams {
    pub fn builder() -> builder::CumulativeMetricTypeParamsCumulativeTypeParams {
        Default::default()
    }
}
#[doc = "`CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg {
    #[serde(rename = "average")]
    Average,
    #[serde(rename = "first")]
    First,
    #[serde(rename = "last")]
    Last,
}
impl ::std::fmt::Display for CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Average => f.write_str("average"),
            Self::First => f.write_str("first"),
            Self::Last => f.write_str("last"),
        }
    }
}
impl ::std::str::FromStr for CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "average" => Ok(Self::Average),
            "first" => Ok(Self::First),
            "last" => Ok(Self::Last),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`CumulativeMetricTypeParamsGrainToDate`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum CumulativeMetricTypeParamsGrainToDate {
    #[serde(rename = "day")]
    Day,
    #[serde(rename = "hour")]
    Hour,
    #[serde(rename = "microsecond")]
    Microsecond,
    #[serde(rename = "millisecond")]
    Millisecond,
    #[serde(rename = "minute")]
    Minute,
    #[serde(rename = "month")]
    Month,
    #[serde(rename = "nanosecond")]
    Nanosecond,
    #[serde(rename = "quarter")]
    Quarter,
    #[serde(rename = "second")]
    Second,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "year")]
    Year,
}
impl ::std::fmt::Display for CumulativeMetricTypeParamsGrainToDate {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Day => f.write_str("day"),
            Self::Hour => f.write_str("hour"),
            Self::Microsecond => f.write_str("microsecond"),
            Self::Millisecond => f.write_str("millisecond"),
            Self::Minute => f.write_str("minute"),
            Self::Month => f.write_str("month"),
            Self::Nanosecond => f.write_str("nanosecond"),
            Self::Quarter => f.write_str("quarter"),
            Self::Second => f.write_str("second"),
            Self::Week => f.write_str("week"),
            Self::Year => f.write_str("year"),
        }
    }
}
impl ::std::str::FromStr for CumulativeMetricTypeParamsGrainToDate {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "day" => Ok(Self::Day),
            "hour" => Ok(Self::Hour),
            "microsecond" => Ok(Self::Microsecond),
            "millisecond" => Ok(Self::Millisecond),
            "minute" => Ok(Self::Minute),
            "month" => Ok(Self::Month),
            "nanosecond" => Ok(Self::Nanosecond),
            "quarter" => Ok(Self::Quarter),
            "second" => Ok(Self::Second),
            "week" => Ok(Self::Week),
            "year" => Ok(Self::Year),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CumulativeMetricTypeParamsGrainToDate {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CumulativeMetricTypeParamsGrainToDate {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DataTests`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DataTests {
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_0: ::std::option::Option<::std::string::String>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_1: ::std::option::Option<DataTestsSubtype1>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_2: ::std::option::Option<RelationshipsTest>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_3: ::std::option::Option<AcceptedValuesTest>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_4: ::std::option::Option<
        ::std::collections::HashMap<DataTestsSubtype4Key, DataTestsSubtype4Value>,
    >,
}
impl DataTests {
    pub fn builder() -> builder::DataTests {
        Default::default()
    }
}
#[doc = "`DataTestsSubtype1`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DataTestsSubtype1 {
    #[serde(rename = "not_null")]
    NotNull,
    #[serde(rename = "unique")]
    Unique,
}
impl ::std::fmt::Display for DataTestsSubtype1 {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::NotNull => f.write_str("not_null"),
            Self::Unique => f.write_str("unique"),
        }
    }
}
impl ::std::str::FromStr for DataTestsSubtype1 {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "not_null" => Ok(Self::NotNull),
            "unique" => Ok(Self::Unique),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DataTestsSubtype1 {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DataTestsSubtype1 {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DataTestsSubtype4Key`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DataTestsSubtype4Key(::std::string::String);
impl ::std::ops::Deref for DataTestsSubtype4Key {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DataTestsSubtype4Key> for ::std::string::String {
    fn from(value: DataTestsSubtype4Key) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DataTestsSubtype4Key {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \".*\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for DataTestsSubtype4Key {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DataTestsSubtype4Key {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DataTestsSubtype4Key {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`DataTestsSubtype4Value`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DataTestsSubtype4Value {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<TestConfigs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "where",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub where_: ::std::option::Option<::std::string::String>,
}
impl DataTestsSubtype4Value {
    pub fn builder() -> builder::DataTestsSubtype4Value {
        Default::default()
    }
}
#[doc = "`DbtYmlFiles`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFiles {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub analyses: ::std::vec::Vec<DbtYmlFilesAnalysesItem>,
    #[doc = "Add descriptions to custom singular tests"]
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub data_tests: ::std::vec::Vec<DbtYmlFilesDataTestsItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub exposures: ::std::vec::Vec<DbtYmlFilesExposuresItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub groups: ::std::vec::Vec<DbtYmlFilesGroupsItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub macros: ::std::vec::Vec<DbtYmlFilesMacrosItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub metrics: ::std::vec::Vec<DbtYmlFilesMetricsItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub models: ::std::vec::Vec<DbtYmlFilesModelsItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub saved_queries: ::std::vec::Vec<DbtYmlFilesSavedQueriesItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub seeds: ::std::vec::Vec<DbtYmlFilesSeedsItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub semantic_models: ::std::vec::Vec<DbtYmlFilesSemanticModelsItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub snapshots: ::std::vec::Vec<DbtYmlFilesSnapshotsItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub sources: ::std::vec::Vec<DbtYmlFilesSourcesItem>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub unit_tests: ::std::vec::Vec<DbtYmlFilesUnitTestsItem>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub version: ::std::option::Option<f64>,
}
impl DbtYmlFiles {
    pub fn builder() -> builder::DbtYmlFiles {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesAnalysesItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DbtYmlFilesAnalysesItem {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub columns: ::std::vec::Vec<DbtYmlFilesAnalysesItemColumnsItem>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DbtYmlFilesAnalysesItemConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    pub name: ::std::string::String,
}
impl DbtYmlFilesAnalysesItem {
    pub fn builder() -> builder::DbtYmlFilesAnalysesItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesAnalysesItemColumnsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesAnalysesItemColumnsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub data_type: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    pub name: ::std::string::String,
}
impl DbtYmlFilesAnalysesItemColumnsItem {
    pub fn builder() -> builder::DbtYmlFilesAnalysesItemColumnsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesAnalysesItemConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesAnalysesItemConfig {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
}
impl DbtYmlFilesAnalysesItemConfig {
    pub fn builder() -> builder::DbtYmlFilesAnalysesItemConfig {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesDataTestsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesDataTestsItem {
    #[doc = "A description of the test's purpose, how it is implemented, and perhaps what you should do if it starts failing."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[doc = "The name of the singular test"]
    pub name: ::std::string::String,
}
impl DbtYmlFilesDataTestsItem {
    pub fn builder() -> builder::DbtYmlFilesDataTestsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesExposuresItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DbtYmlFilesExposuresItem {
    pub depends_on: ::std::vec::Vec<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub maturity: ::std::option::Option<DbtYmlFilesExposuresItemMaturity>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    pub name: ::std::string::String,
    pub owner: DbtYmlFilesExposuresItemOwner,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(rename = "type")]
    pub type_: DbtYmlFilesExposuresItemType,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub url: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesExposuresItem {
    pub fn builder() -> builder::DbtYmlFilesExposuresItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesExposuresItemMaturity`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DbtYmlFilesExposuresItemMaturity {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
}
impl ::std::fmt::Display for DbtYmlFilesExposuresItemMaturity {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::High => f.write_str("high"),
            Self::Low => f.write_str("low"),
            Self::Medium => f.write_str("medium"),
        }
    }
}
impl ::std::str::FromStr for DbtYmlFilesExposuresItemMaturity {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "high" => Ok(Self::High),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesExposuresItemMaturity {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesExposuresItemMaturity {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DbtYmlFilesExposuresItemOwner`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged, deny_unknown_fields)]
pub enum DbtYmlFilesExposuresItemOwner {
    Variant0 { email: ::std::string::String },
    Variant1 { name: ::std::string::String },
}
#[doc = "`DbtYmlFilesExposuresItemType`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DbtYmlFilesExposuresItemType {
    #[serde(rename = "analysis")]
    Analysis,
    #[serde(rename = "application")]
    Application,
    #[serde(rename = "dashboard")]
    Dashboard,
    #[serde(rename = "ml")]
    Ml,
    #[serde(rename = "notebook")]
    Notebook,
}
impl ::std::fmt::Display for DbtYmlFilesExposuresItemType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Analysis => f.write_str("analysis"),
            Self::Application => f.write_str("application"),
            Self::Dashboard => f.write_str("dashboard"),
            Self::Ml => f.write_str("ml"),
            Self::Notebook => f.write_str("notebook"),
        }
    }
}
impl ::std::str::FromStr for DbtYmlFilesExposuresItemType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "analysis" => Ok(Self::Analysis),
            "application" => Ok(Self::Application),
            "dashboard" => Ok(Self::Dashboard),
            "ml" => Ok(Self::Ml),
            "notebook" => Ok(Self::Notebook),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesExposuresItemType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesExposuresItemType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DbtYmlFilesGroupsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesGroupsItem {
    pub name: ::std::string::String,
    pub owner: DbtYmlFilesGroupsItemOwner,
}
impl DbtYmlFilesGroupsItem {
    pub fn builder() -> builder::DbtYmlFilesGroupsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesGroupsItemOwner`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesGroupsItemOwner {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub email: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesGroupsItemOwner {
    pub fn builder() -> builder::DbtYmlFilesGroupsItemOwner {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesMacrosItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesMacrosItem {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub arguments: ::std::vec::Vec<DbtYmlFilesMacrosItemArgumentsItem>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    pub name: ::std::string::String,
}
impl DbtYmlFilesMacrosItem {
    pub fn builder() -> builder::DbtYmlFilesMacrosItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesMacrosItemArgumentsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesMacrosItemArgumentsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    pub name: ::std::string::String,
    #[serde(
        rename = "type",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub type_: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesMacrosItemArgumentsItem {
    pub fn builder() -> builder::DbtYmlFilesMacrosItemArgumentsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesMetricsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesMetricsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub filter: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    pub label: ::std::string::String,
    pub name: DbtYmlFilesMetricsItemName,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub time_granularity: ::std::option::Option<::std::string::String>,
    #[serde(rename = "type")]
    pub type_: DbtYmlFilesMetricsItemType,
    pub type_params: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl DbtYmlFilesMetricsItem {
    pub fn builder() -> builder::DbtYmlFilesMetricsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesMetricsItemName`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DbtYmlFilesMetricsItemName(::std::string::String);
impl ::std::ops::Deref for DbtYmlFilesMetricsItemName {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DbtYmlFilesMetricsItemName> for ::std::string::String {
    fn from(value: DbtYmlFilesMetricsItemName) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DbtYmlFilesMetricsItemName {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesMetricsItemName {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesMetricsItemName {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DbtYmlFilesMetricsItemName {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`DbtYmlFilesMetricsItemType`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DbtYmlFilesMetricsItemType {
    #[serde(rename = "CONVERSION")]
    Conversion,
    #[serde(rename = "CUMULATIVE")]
    Cumulative,
    #[serde(rename = "DERIVED")]
    Derived,
    #[serde(rename = "RATIO")]
    Ratio,
    #[serde(rename = "SIMPLE")]
    Simple,
}
impl ::std::fmt::Display for DbtYmlFilesMetricsItemType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Conversion => f.write_str("CONVERSION"),
            Self::Cumulative => f.write_str("CUMULATIVE"),
            Self::Derived => f.write_str("DERIVED"),
            Self::Ratio => f.write_str("RATIO"),
            Self::Simple => f.write_str("SIMPLE"),
        }
    }
}
impl ::std::str::FromStr for DbtYmlFilesMetricsItemType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "CONVERSION" => Ok(Self::Conversion),
            "CUMULATIVE" => Ok(Self::Cumulative),
            "DERIVED" => Ok(Self::Derived),
            "RATIO" => Ok(Self::Ratio),
            "SIMPLE" => Ok(Self::Simple),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesMetricsItemType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesMetricsItemType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DbtYmlFilesModelsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesModelsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub access: ::std::option::Option<DbtYmlFilesModelsItemAccess>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub columns: ::std::vec::Vec<ColumnProperties>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<ModelConfigs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub constraints: ::std::option::Option<Constraints>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub data_tests: ::std::vec::Vec<DataTests>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub deprecation_date: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub latest_version: ::std::option::Option<f64>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    pub name: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub tests: ::std::vec::Vec<DataTests>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub time_spine: ::std::option::Option<DbtYmlFilesModelsItemTimeSpine>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub versions: ::std::vec::Vec<DbtYmlFilesModelsItemVersionsItem>,
}
impl DbtYmlFilesModelsItem {
    pub fn builder() -> builder::DbtYmlFilesModelsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesModelsItemAccess`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DbtYmlFilesModelsItemAccess {
    #[serde(rename = "private")]
    Private,
    #[serde(rename = "protected")]
    Protected,
    #[serde(rename = "public")]
    Public,
}
impl ::std::fmt::Display for DbtYmlFilesModelsItemAccess {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Private => f.write_str("private"),
            Self::Protected => f.write_str("protected"),
            Self::Public => f.write_str("public"),
        }
    }
}
impl ::std::str::FromStr for DbtYmlFilesModelsItemAccess {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "private" => Ok(Self::Private),
            "protected" => Ok(Self::Protected),
            "public" => Ok(Self::Public),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesModelsItemAccess {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesModelsItemAccess {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DbtYmlFilesModelsItemTimeSpine`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesModelsItemTimeSpine {
    pub standard_granularity_column: ::std::string::String,
}
impl DbtYmlFilesModelsItemTimeSpine {
    pub fn builder() -> builder::DbtYmlFilesModelsItemTimeSpine {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesModelsItemVersionsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DbtYmlFilesModelsItemVersionsItem {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub columns: ::std::vec::Vec<DbtYmlFilesModelsItemVersionsItemColumnsItem>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<ModelConfigs>,
    pub v: f64,
}
impl DbtYmlFilesModelsItemVersionsItem {
    pub fn builder() -> builder::DbtYmlFilesModelsItemVersionsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesModelsItemVersionsItemColumnsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesModelsItemVersionsItemColumnsItem {
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_0: ::std::option::Option<IncludeExclude>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_1: ::std::option::Option<ColumnProperties>,
}
impl DbtYmlFilesModelsItemVersionsItemColumnsItem {
    pub fn builder() -> builder::DbtYmlFilesModelsItemVersionsItemColumnsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSavedQueriesItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSavedQueriesItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DbtYmlFilesSavedQueriesItemConfig>,
    pub description: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub exports: ::std::vec::Vec<Export>,
    pub label: ::std::string::String,
    pub name: ::std::string::String,
    pub query_params: DbtYmlFilesSavedQueriesItemQueryParams,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<DbtYmlFilesSavedQueriesItemTags>,
}
impl DbtYmlFilesSavedQueriesItem {
    pub fn builder() -> builder::DbtYmlFilesSavedQueriesItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSavedQueriesItemConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesSavedQueriesItemConfig {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub cache: ::std::option::Option<DbtYmlFilesSavedQueriesItemConfigCache>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub export_as: ::std::option::Option<DbtYmlFilesSavedQueriesItemConfigExportAs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesSavedQueriesItemConfig {
    pub fn builder() -> builder::DbtYmlFilesSavedQueriesItemConfig {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSavedQueriesItemConfigCache`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSavedQueriesItemConfigCache {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<bool>,
}
impl DbtYmlFilesSavedQueriesItemConfigCache {
    pub fn builder() -> builder::DbtYmlFilesSavedQueriesItemConfigCache {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSavedQueriesItemConfigExportAs`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DbtYmlFilesSavedQueriesItemConfigExportAs {
    #[serde(rename = "table")]
    Table,
    #[serde(rename = "view")]
    View,
}
impl ::std::fmt::Display for DbtYmlFilesSavedQueriesItemConfigExportAs {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Table => f.write_str("table"),
            Self::View => f.write_str("view"),
        }
    }
}
impl ::std::str::FromStr for DbtYmlFilesSavedQueriesItemConfigExportAs {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "table" => Ok(Self::Table),
            "view" => Ok(Self::View),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesSavedQueriesItemConfigExportAs {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesSavedQueriesItemConfigExportAs {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DbtYmlFilesSavedQueriesItemQueryParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesSavedQueriesItemQueryParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub dimensions: ::std::option::Option<ArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub metrics: ::std::option::Option<ArrayOfStrings>,
    #[serde(
        rename = "where",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub where_: ::std::option::Option<ArrayOfStrings>,
}
impl DbtYmlFilesSavedQueriesItemQueryParams {
    pub fn builder() -> builder::DbtYmlFilesSavedQueriesItemQueryParams {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSavedQueriesItemTags`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum DbtYmlFilesSavedQueriesItemTags {
    Array(::std::vec::Vec<::std::string::String>),
    String(::std::string::String),
}
impl ::std::convert::From<::std::vec::Vec<::std::string::String>>
    for DbtYmlFilesSavedQueriesItemTags
{
    fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
        Self::Array(value)
    }
}
#[doc = "`DbtYmlFilesSeedsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSeedsItem {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub columns: ::std::vec::Vec<ColumnProperties>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DbtYmlFilesSeedsItemConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    pub name: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub tests: ::std::vec::Vec<DataTests>,
}
impl DbtYmlFilesSeedsItem {
    pub fn builder() -> builder::DbtYmlFilesSeedsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSeedsItemConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesSeedsItemConfig {
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub column_types: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub copy_grants: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub data_tests: ::std::vec::Vec<DataTests>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grants: ::std::option::Option<Grants>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quote_columns: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesSeedsItemConfig {
    pub fn builder() -> builder::DbtYmlFilesSeedsItemConfig {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSemanticModelsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSemanticModelsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub defaults: ::std::option::Option<DbtYmlFilesSemanticModelsItemDefaults>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub dimensions: ::std::vec::Vec<Dimension>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub entities: ::std::vec::Vec<Entity>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub measures: ::std::vec::Vec<Measure>,
    pub model: ::std::string::String,
    pub name: DbtYmlFilesSemanticModelsItemName,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub primary_entity: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesSemanticModelsItem {
    pub fn builder() -> builder::DbtYmlFilesSemanticModelsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSemanticModelsItemDefaults`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSemanticModelsItemDefaults {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub agg_time_dimension: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesSemanticModelsItemDefaults {
    pub fn builder() -> builder::DbtYmlFilesSemanticModelsItemDefaults {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSemanticModelsItemName`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DbtYmlFilesSemanticModelsItemName(::std::string::String);
impl ::std::ops::Deref for DbtYmlFilesSemanticModelsItemName {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DbtYmlFilesSemanticModelsItemName> for ::std::string::String {
    fn from(value: DbtYmlFilesSemanticModelsItemName) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DbtYmlFilesSemanticModelsItemName {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesSemanticModelsItemName {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesSemanticModelsItemName {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DbtYmlFilesSemanticModelsItemName {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`DbtYmlFilesSnapshotsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSnapshotsItem {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub columns: ::std::vec::Vec<ColumnProperties>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DbtYmlFilesSnapshotsItemConfig>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub data_tests: ::std::vec::Vec<DataTests>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    pub name: ::std::string::String,
    #[doc = "Required when defining your snapshot in YAML instead of using a legacy snapshot block."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub relation: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub tests: ::std::vec::Vec<DataTests>,
}
impl DbtYmlFilesSnapshotsItem {
    pub fn builder() -> builder::DbtYmlFilesSnapshotsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSnapshotsItemConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesSnapshotsItemConfig {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub alias: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub check_cols: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grants: ::std::option::Option<Grants>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub persist_docs: ::std::option::Option<PersistDocsConfig>,
    #[serde(
        rename = "post-hook",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub post_hook: ::std::option::Option<ArrayOfStrings>,
    #[serde(
        rename = "pre-hook",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pre_hook: ::std::option::Option<ArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quote_columns: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub strategy: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub target_database: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub target_schema: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub unique_key: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub updated_at: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesSnapshotsItemConfig {
    pub fn builder() -> builder::DbtYmlFilesSnapshotsItemConfig {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSourcesItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DbtYmlFilesSourcesItem {
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub config: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub data_tests: ::std::vec::Vec<DataTests>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness: ::std::option::Option<FreshnessDefinition>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub loaded_at_field: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub loader: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "How you will identify the schema in {{ source() }} calls. Unless `schema` is also set, this will be the name of the schema in the database."]
    pub name: ::std::string::String,
    #[doc = "The name of another package installed in your project. If that package has a source with the same name as this one, its properties will be applied on top of the base properties of the overridden source. https://docs.getdbt.com/reference/resource-properties/overrides"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub overrides: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quoting: ::std::option::Option<DbtYmlFilesSourcesItemQuoting>,
    #[doc = "The schema name as stored in the database. Only needed if you want to use a different `name` than what exists in the database (otherwise `name` is used by default)"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub tables: ::std::vec::Vec<DbtYmlFilesSourcesItemTablesItem>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub tests: ::std::vec::Vec<DataTests>,
}
impl DbtYmlFilesSourcesItem {
    pub fn builder() -> builder::DbtYmlFilesSourcesItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSourcesItemQuoting`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSourcesItemQuoting {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub identifier: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<BooleanOrJinjaString>,
}
impl DbtYmlFilesSourcesItemQuoting {
    pub fn builder() -> builder::DbtYmlFilesSourcesItemQuoting {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSourcesItemTablesItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSourcesItemTablesItem {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub columns: ::std::vec::Vec<ColumnProperties>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DbtYmlFilesSourcesItemTablesItemConfig>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub data_tests: ::std::vec::Vec<DataTests>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub external: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness: ::std::option::Option<FreshnessDefinition>,
    #[doc = "The table name as stored in the database. Only needed if you want to give the source a different name than what exists in the database (otherwise `name` is used by default)"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub identifier: ::std::option::Option<::std::string::String>,
    #[doc = "Which column to check during data freshness tests. Only needed if the table has a different loaded_at_field to the one defined on the source overall."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub loaded_at_field: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub loader: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "How you will identify the table in {{ source() }} calls. Unless `identifier` is also set, this will be the name of the table in the database."]
    pub name: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quoting: ::std::option::Option<DbtYmlFilesSourcesItemTablesItemQuoting>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub tests: ::std::vec::Vec<DataTests>,
}
impl DbtYmlFilesSourcesItemTablesItem {
    pub fn builder() -> builder::DbtYmlFilesSourcesItemTablesItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSourcesItemTablesItemConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesSourcesItemTablesItemConfig {
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub additional_properties: ::std::option::Option<::serde_json::Value>,
    #[doc = "The column that represents the actual moment the row's event happened. Contrast with, for example, when it was loaded to the warehouse."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub event_time: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesSourcesItemTablesItemConfig {
    pub fn builder() -> builder::DbtYmlFilesSourcesItemTablesItemConfig {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesSourcesItemTablesItemQuoting`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesSourcesItemTablesItemQuoting {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub identifier: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<BooleanOrJinjaString>,
}
impl DbtYmlFilesSourcesItemTablesItemQuoting {
    pub fn builder() -> builder::DbtYmlFilesSourcesItemTablesItemQuoting {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesUnitTestsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesUnitTestsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DbtYmlFilesUnitTestsItemConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    pub expect: DbtYmlFilesUnitTestsItemExpect,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub given: ::std::vec::Vec<DbtYmlFilesUnitTestsItemGivenItem>,
    #[doc = "The name of the model whose behaviour you are testing. Does not need to be wrapped in a ref."]
    pub model: ::std::string::String,
    pub name: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub overrides: ::std::option::Option<DbtYmlFilesUnitTestsItemOverrides>,
}
impl DbtYmlFilesUnitTestsItem {
    pub fn builder() -> builder::DbtYmlFilesUnitTestsItem {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesUnitTestsItemConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesUnitTestsItemConfig {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
}
impl DbtYmlFilesUnitTestsItemConfig {
    pub fn builder() -> builder::DbtYmlFilesUnitTestsItemConfig {
        Default::default()
    }
}
#[doc = "`DbtYmlFilesUnitTestsItemExpect`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesUnitTestsItemExpect {
    #[doc = "Only relevant when format is csv. Specify the name of a fixture instead of providing `rows`."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub fixture: ::std::option::Option<::std::string::String>,
    #[doc = "Defaults to `dict` when not specified"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub format: ::std::option::Option<DbtYmlFilesUnitTestsItemExpectFormat>,
}
impl DbtYmlFilesUnitTestsItemExpect {
    pub fn builder() -> builder::DbtYmlFilesUnitTestsItemExpect {
        Default::default()
    }
}
#[doc = "Defaults to `dict` when not specified"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DbtYmlFilesUnitTestsItemExpectFormat {
    #[serde(rename = "csv")]
    Csv,
    #[serde(rename = "dict")]
    Dict,
    #[serde(rename = "sql")]
    Sql,
}
impl ::std::fmt::Display for DbtYmlFilesUnitTestsItemExpectFormat {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Csv => f.write_str("csv"),
            Self::Dict => f.write_str("dict"),
            Self::Sql => f.write_str("sql"),
        }
    }
}
impl ::std::str::FromStr for DbtYmlFilesUnitTestsItemExpectFormat {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "csv" => Ok(Self::Csv),
            "dict" => Ok(Self::Dict),
            "sql" => Ok(Self::Sql),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesUnitTestsItemExpectFormat {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesUnitTestsItemExpectFormat {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DbtYmlFilesUnitTestsItemGivenItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtYmlFilesUnitTestsItemGivenItem {
    #[doc = "Only relevant when format is csv. Specify the name of a fixture instead of providing `rows`."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub fixture: ::std::option::Option<::std::string::String>,
    #[doc = "Defaults to `dict` when not specified"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub format: ::std::option::Option<DbtYmlFilesUnitTestsItemGivenItemFormat>,
    #[doc = "The relation whose inputs you need to mock. Enclose in ref or source without curly braces"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub input: ::std::option::Option<::std::string::String>,
}
impl DbtYmlFilesUnitTestsItemGivenItem {
    pub fn builder() -> builder::DbtYmlFilesUnitTestsItemGivenItem {
        Default::default()
    }
}
#[doc = "Defaults to `dict` when not specified"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DbtYmlFilesUnitTestsItemGivenItemFormat {
    #[serde(rename = "csv")]
    Csv,
    #[serde(rename = "dict")]
    Dict,
    #[serde(rename = "sql")]
    Sql,
}
impl ::std::fmt::Display for DbtYmlFilesUnitTestsItemGivenItemFormat {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Csv => f.write_str("csv"),
            Self::Dict => f.write_str("dict"),
            Self::Sql => f.write_str("sql"),
        }
    }
}
impl ::std::str::FromStr for DbtYmlFilesUnitTestsItemGivenItemFormat {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "csv" => Ok(Self::Csv),
            "dict" => Ok(Self::Dict),
            "sql" => Ok(Self::Sql),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DbtYmlFilesUnitTestsItemGivenItemFormat {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DbtYmlFilesUnitTestsItemGivenItemFormat {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DbtYmlFilesUnitTestsItemOverrides`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DbtYmlFilesUnitTestsItemOverrides {
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub env_vars: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub macros: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub vars: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl DbtYmlFilesUnitTestsItemOverrides {
    pub fn builder() -> builder::DbtYmlFilesUnitTestsItemOverrides {
        Default::default()
    }
}
#[doc = "`DerivedMetricTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DerivedMetricTypeParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub expr: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub metrics: ::std::vec::Vec<MetricInputSchema>,
}
impl DerivedMetricTypeParams {
    pub fn builder() -> builder::DerivedMetricTypeParams {
        Default::default()
    }
}
#[doc = "`Dimension`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct Dimension {
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_0: ::std::option::Option<DimensionSubtype0>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_1: ::std::option::Option<DimensionSubtype1>,
}
impl Dimension {
    pub fn builder() -> builder::Dimension {
        Default::default()
    }
}
#[doc = "`DimensionSubtype0`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DimensionSubtype0 {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DimensionSubtype0Config>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub expr: ::std::option::Option<DimensionSubtype0Expr>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub is_partition: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub label: ::std::option::Option<::std::string::String>,
    pub name: DimensionSubtype0Name,
    #[serde(rename = "type")]
    pub type_: DimensionSubtype0Type,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub type_params: ::std::option::Option<DimensionTypeParams>,
}
impl DimensionSubtype0 {
    pub fn builder() -> builder::DimensionSubtype0 {
        Default::default()
    }
}
#[doc = "`DimensionSubtype0Config`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DimensionSubtype0Config {
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl DimensionSubtype0Config {
    pub fn builder() -> builder::DimensionSubtype0Config {
        Default::default()
    }
}
#[doc = "`DimensionSubtype0Expr`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum DimensionSubtype0Expr {
    Boolean(bool),
    String(::std::string::String),
}
impl ::std::fmt::Display for DimensionSubtype0Expr {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Boolean(x) => x.fmt(f),
            Self::String(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<bool> for DimensionSubtype0Expr {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
#[doc = "`DimensionSubtype0Name`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DimensionSubtype0Name(::std::string::String);
impl ::std::ops::Deref for DimensionSubtype0Name {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DimensionSubtype0Name> for ::std::string::String {
    fn from(value: DimensionSubtype0Name) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DimensionSubtype0Name {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for DimensionSubtype0Name {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DimensionSubtype0Name {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DimensionSubtype0Name {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`DimensionSubtype0Type`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DimensionSubtype0Type {
    #[serde(rename = "CATEGORICAL")]
    Categorical,
    #[serde(rename = "TIME")]
    Time,
}
impl ::std::fmt::Display for DimensionSubtype0Type {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Categorical => f.write_str("CATEGORICAL"),
            Self::Time => f.write_str("TIME"),
        }
    }
}
impl ::std::str::FromStr for DimensionSubtype0Type {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "CATEGORICAL" => Ok(Self::Categorical),
            "TIME" => Ok(Self::Time),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DimensionSubtype0Type {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DimensionSubtype0Type {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DimensionSubtype1`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DimensionSubtype1 {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<DimensionSubtype1Config>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub expr: ::std::option::Option<DimensionSubtype1Expr>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub is_partition: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub label: ::std::option::Option<::std::string::String>,
    pub name: DimensionSubtype1Name,
    #[serde(rename = "type")]
    pub type_: DimensionSubtype1Type,
    pub type_params: DimensionTypeParams,
}
impl DimensionSubtype1 {
    pub fn builder() -> builder::DimensionSubtype1 {
        Default::default()
    }
}
#[doc = "`DimensionSubtype1Config`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DimensionSubtype1Config {
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl DimensionSubtype1Config {
    pub fn builder() -> builder::DimensionSubtype1Config {
        Default::default()
    }
}
#[doc = "`DimensionSubtype1Expr`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum DimensionSubtype1Expr {
    Boolean(bool),
    String(::std::string::String),
}
impl ::std::fmt::Display for DimensionSubtype1Expr {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Boolean(x) => x.fmt(f),
            Self::String(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<bool> for DimensionSubtype1Expr {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
#[doc = "`DimensionSubtype1Name`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DimensionSubtype1Name(::std::string::String);
impl ::std::ops::Deref for DimensionSubtype1Name {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DimensionSubtype1Name> for ::std::string::String {
    fn from(value: DimensionSubtype1Name) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DimensionSubtype1Name {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for DimensionSubtype1Name {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DimensionSubtype1Name {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DimensionSubtype1Name {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`DimensionSubtype1Type`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DimensionSubtype1Type {
    #[serde(rename = "TIME")]
    Time,
}
impl ::std::fmt::Display for DimensionSubtype1Type {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Time => f.write_str("TIME"),
        }
    }
}
impl ::std::str::FromStr for DimensionSubtype1Type {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "TIME" => Ok(Self::Time),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DimensionSubtype1Type {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DimensionSubtype1Type {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DimensionTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DimensionTypeParams {
    pub time_granularity: DimensionTypeParamsTimeGranularity,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub validity_params: ::std::option::Option<ValidityParams>,
}
impl DimensionTypeParams {
    pub fn builder() -> builder::DimensionTypeParams {
        Default::default()
    }
}
#[doc = "`DimensionTypeParamsTimeGranularity`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DimensionTypeParamsTimeGranularity {
    #[serde(rename = "DAY")]
    Day,
    #[serde(rename = "HOUR")]
    Hour,
    #[serde(rename = "MICROSECOND")]
    Microsecond,
    #[serde(rename = "MILLISECOND")]
    Millisecond,
    #[serde(rename = "MINUTE")]
    Minute,
    #[serde(rename = "MONTH")]
    Month,
    #[serde(rename = "NANOSECOND")]
    Nanosecond,
    #[serde(rename = "QUARTER")]
    Quarter,
    #[serde(rename = "SECOND")]
    Second,
    #[serde(rename = "WEEK")]
    Week,
    #[serde(rename = "YEAR")]
    Year,
}
impl ::std::fmt::Display for DimensionTypeParamsTimeGranularity {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Day => f.write_str("DAY"),
            Self::Hour => f.write_str("HOUR"),
            Self::Microsecond => f.write_str("MICROSECOND"),
            Self::Millisecond => f.write_str("MILLISECOND"),
            Self::Minute => f.write_str("MINUTE"),
            Self::Month => f.write_str("MONTH"),
            Self::Nanosecond => f.write_str("NANOSECOND"),
            Self::Quarter => f.write_str("QUARTER"),
            Self::Second => f.write_str("SECOND"),
            Self::Week => f.write_str("WEEK"),
            Self::Year => f.write_str("YEAR"),
        }
    }
}
impl ::std::str::FromStr for DimensionTypeParamsTimeGranularity {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "DAY" => Ok(Self::Day),
            "HOUR" => Ok(Self::Hour),
            "MICROSECOND" => Ok(Self::Microsecond),
            "MILLISECOND" => Ok(Self::Millisecond),
            "MINUTE" => Ok(Self::Minute),
            "MONTH" => Ok(Self::Month),
            "NANOSECOND" => Ok(Self::Nanosecond),
            "QUARTER" => Ok(Self::Quarter),
            "SECOND" => Ok(Self::Second),
            "WEEK" => Ok(Self::Week),
            "YEAR" => Ok(Self::Year),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DimensionTypeParamsTimeGranularity {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DimensionTypeParamsTimeGranularity {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Configurations for the appearance of nodes in the dbt documentation."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct DocsConfig {
    #[doc = "The color of the node on the DAG in the documentation. It must be an Hex code or a valid CSS color name."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub node_color: ::std::option::Option<DocsConfigNodeColor>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub show: ::std::option::Option<bool>,
}
impl DocsConfig {
    pub fn builder() -> builder::DocsConfig {
        Default::default()
    }
}
#[doc = "The color of the node on the DAG in the documentation. It must be an Hex code or a valid CSS color name."]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DocsConfigNodeColor(::std::string::String);
impl ::std::ops::Deref for DocsConfigNodeColor {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DocsConfigNodeColor> for ::std::string::String {
    fn from(value: DocsConfigNodeColor) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DocsConfigNodeColor {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("^(#[a-fA-F0-9]{3}|#[a-fA-F0-9]{6}|[^#][a-zA-Z]*)$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err(
                "doesn't match pattern \"^(#[a-fA-F0-9]{3}|#[a-fA-F0-9]{6}|[^#][a-zA-Z]*)$\""
                    .into(),
            );
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for DocsConfigNodeColor {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DocsConfigNodeColor {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DocsConfigNodeColor {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`Entity`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<EntityConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub entity: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub expr: ::std::option::Option<EntityExpr>,
    pub name: EntityName,
    #[serde(rename = "type")]
    pub type_: EntityType,
}
impl Entity {
    pub fn builder() -> builder::Entity {
        Default::default()
    }
}
#[doc = "`EntityConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct EntityConfig {
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl EntityConfig {
    pub fn builder() -> builder::EntityConfig {
        Default::default()
    }
}
#[doc = "`EntityExpr`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum EntityExpr {
    Boolean(bool),
    String(::std::string::String),
}
impl ::std::fmt::Display for EntityExpr {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Boolean(x) => x.fmt(f),
            Self::String(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<bool> for EntityExpr {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
#[doc = "`EntityName`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct EntityName(::std::string::String);
impl ::std::ops::Deref for EntityName {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<EntityName> for ::std::string::String {
    fn from(value: EntityName) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for EntityName {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for EntityName {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EntityName {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for EntityName {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`EntityType`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum EntityType {
    #[serde(rename = "FOREIGN")]
    Foreign,
    #[serde(rename = "NATURAL")]
    Natural,
    #[serde(rename = "PRIMARY")]
    Primary,
    #[serde(rename = "UNIQUE")]
    Unique,
}
impl ::std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Foreign => f.write_str("FOREIGN"),
            Self::Natural => f.write_str("NATURAL"),
            Self::Primary => f.write_str("PRIMARY"),
            Self::Unique => f.write_str("UNIQUE"),
        }
    }
}
impl ::std::str::FromStr for EntityType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "FOREIGN" => Ok(Self::Foreign),
            "NATURAL" => Ok(Self::Natural),
            "PRIMARY" => Ok(Self::Primary),
            "UNIQUE" => Ok(Self::Unique),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EntityType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EntityType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Export`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Export {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<ExportConfig>,
    pub name: ::std::string::String,
}
impl Export {
    pub fn builder() -> builder::Export {
        Default::default()
    }
}
#[doc = "`ExportConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct ExportConfig {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub alias: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub export_as: ::std::option::Option<ExportConfigExportAs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<::std::string::String>,
}
impl ExportConfig {
    pub fn builder() -> builder::ExportConfig {
        Default::default()
    }
}
#[doc = "`ExportConfigExportAs`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ExportConfigExportAs {
    #[serde(rename = "cache")]
    Cache,
    #[serde(rename = "table")]
    Table,
    #[serde(rename = "view")]
    View,
}
impl ::std::fmt::Display for ExportConfigExportAs {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Cache => f.write_str("cache"),
            Self::Table => f.write_str("table"),
            Self::View => f.write_str("view"),
        }
    }
}
impl ::std::str::FromStr for ExportConfigExportAs {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "cache" => Ok(Self::Cache),
            "table" => Ok(Self::Table),
            "view" => Ok(Self::View),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ExportConfigExportAs {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ExportConfigExportAs {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`FreshnessDefinition`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct FreshnessDefinition {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub error_after: ::std::option::Option<FreshnessRules>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub filter: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub warn_after: ::std::option::Option<FreshnessRules>,
}
impl FreshnessDefinition {
    pub fn builder() -> builder::FreshnessDefinition {
        Default::default()
    }
}
#[doc = "`FreshnessRules`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FreshnessRules {
    pub count: NumberOrJinjaString,
    pub period: FreshnessRulesPeriod,
}
impl FreshnessRules {
    pub fn builder() -> builder::FreshnessRules {
        Default::default()
    }
}
#[doc = "`FreshnessRulesPeriod`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum FreshnessRulesPeriod {
    #[serde(rename = "day")]
    Day,
    #[serde(rename = "hour")]
    Hour,
    #[serde(rename = "minute")]
    Minute,
}
impl ::std::fmt::Display for FreshnessRulesPeriod {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Day => f.write_str("day"),
            Self::Hour => f.write_str("hour"),
            Self::Minute => f.write_str("minute"),
        }
    }
}
impl ::std::str::FromStr for FreshnessRulesPeriod {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "day" => Ok(Self::Day),
            "hour" => Ok(Self::Hour),
            "minute" => Ok(Self::Minute),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for FreshnessRulesPeriod {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for FreshnessRulesPeriod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "grant config. each key is a database permission and the value is the grantee of that permission"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Grants(pub ::std::collections::HashMap<GrantsKey, StringOrArrayOfStrings>);
impl ::std::ops::Deref for Grants {
    type Target = ::std::collections::HashMap<GrantsKey, StringOrArrayOfStrings>;
    fn deref(&self) -> &::std::collections::HashMap<GrantsKey, StringOrArrayOfStrings> {
        &self.0
    }
}
impl ::std::convert::From<Grants>
    for ::std::collections::HashMap<GrantsKey, StringOrArrayOfStrings>
{
    fn from(value: Grants) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::collections::HashMap<GrantsKey, StringOrArrayOfStrings>>
    for Grants
{
    fn from(value: ::std::collections::HashMap<GrantsKey, StringOrArrayOfStrings>) -> Self {
        Self(value)
    }
}
#[doc = "`GrantsKey`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct GrantsKey(::std::string::String);
impl ::std::ops::Deref for GrantsKey {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<GrantsKey> for ::std::string::String {
    fn from(value: GrantsKey) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for GrantsKey {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new(".*").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \".*\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for GrantsKey {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for GrantsKey {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for GrantsKey {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`Group`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
#[serde(transparent)]
pub struct Group(pub ::std::string::String);
impl ::std::ops::Deref for Group {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Group> for ::std::string::String {
    fn from(value: Group) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Group {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Group {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Group {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`IncludeExclude`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct IncludeExclude {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub exclude: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub include: ::std::option::Option<StringOrArrayOfStrings>,
}
impl IncludeExclude {
    pub fn builder() -> builder::IncludeExclude {
        Default::default()
    }
}
#[doc = "`IsTimeDimension`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IsTimeDimension {
    #[serde(rename = "type")]
    pub type_: IsTimeDimensionType,
}
impl IsTimeDimension {
    pub fn builder() -> builder::IsTimeDimension {
        Default::default()
    }
}
#[doc = "`IsTimeDimensionType`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum IsTimeDimensionType {
    #[serde(rename = "TIME")]
    Time,
}
impl ::std::fmt::Display for IsTimeDimensionType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Time => f.write_str("TIME"),
        }
    }
}
impl ::std::str::FromStr for IsTimeDimensionType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "TIME" => Ok(Self::Time),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for IsTimeDimensionType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for IsTimeDimensionType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`JinjaString`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct JinjaString(::std::string::String);
impl ::std::ops::Deref for JinjaString {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<JinjaString> for ::std::string::String {
    fn from(value: JinjaString) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for JinjaString {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\{\\{.*\\}\\}").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"\\{\\{.*\\}\\}\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for JinjaString {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for JinjaString {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for JinjaString {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`Measure`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Measure {
    pub agg: MeasureAgg,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub agg_params: ::std::option::Option<AggregationTypeParams>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub agg_time_dimension: ::std::option::Option<MeasureAggTimeDimension>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<MeasureConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub create_metric: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub create_metric_display_name: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub expr: ::std::option::Option<MeasureExpr>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub label: ::std::option::Option<::std::string::String>,
    pub name: MeasureName,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub non_additive_dimension: ::std::option::Option<NonAdditiveDimension>,
}
impl Measure {
    pub fn builder() -> builder::Measure {
        Default::default()
    }
}
#[doc = "`MeasureAgg`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum MeasureAgg {
    #[serde(rename = "AVERAGE")]
    Average,
    #[serde(rename = "COUNT")]
    Count,
    #[serde(rename = "COUNT_DISTINCT")]
    CountDistinct,
    #[serde(rename = "MAX")]
    Max,
    #[serde(rename = "MEDIAN")]
    Median,
    #[serde(rename = "MIN")]
    Min,
    #[serde(rename = "PERCENTILE")]
    Percentile,
    #[serde(rename = "SUM")]
    Sum,
    #[serde(rename = "SUM_BOOLEAN")]
    SumBoolean,
}
impl ::std::fmt::Display for MeasureAgg {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Average => f.write_str("AVERAGE"),
            Self::Count => f.write_str("COUNT"),
            Self::CountDistinct => f.write_str("COUNT_DISTINCT"),
            Self::Max => f.write_str("MAX"),
            Self::Median => f.write_str("MEDIAN"),
            Self::Min => f.write_str("MIN"),
            Self::Percentile => f.write_str("PERCENTILE"),
            Self::Sum => f.write_str("SUM"),
            Self::SumBoolean => f.write_str("SUM_BOOLEAN"),
        }
    }
}
impl ::std::str::FromStr for MeasureAgg {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "AVERAGE" => Ok(Self::Average),
            "COUNT" => Ok(Self::Count),
            "COUNT_DISTINCT" => Ok(Self::CountDistinct),
            "MAX" => Ok(Self::Max),
            "MEDIAN" => Ok(Self::Median),
            "MIN" => Ok(Self::Min),
            "PERCENTILE" => Ok(Self::Percentile),
            "SUM" => Ok(Self::Sum),
            "SUM_BOOLEAN" => Ok(Self::SumBoolean),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for MeasureAgg {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MeasureAgg {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`MeasureAggTimeDimension`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MeasureAggTimeDimension(::std::string::String);
impl ::std::ops::Deref for MeasureAggTimeDimension {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MeasureAggTimeDimension> for ::std::string::String {
    fn from(value: MeasureAggTimeDimension) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MeasureAggTimeDimension {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for MeasureAggTimeDimension {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MeasureAggTimeDimension {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MeasureAggTimeDimension {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`MeasureConfig`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct MeasureConfig {
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub meta: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl MeasureConfig {
    pub fn builder() -> builder::MeasureConfig {
        Default::default()
    }
}
#[doc = "`MeasureExpr`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum MeasureExpr {
    Boolean(bool),
    Integer(i64),
    String(::std::string::String),
}
impl ::std::fmt::Display for MeasureExpr {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Boolean(x) => x.fmt(f),
            Self::Integer(x) => x.fmt(f),
            Self::String(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<bool> for MeasureExpr {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
impl ::std::convert::From<i64> for MeasureExpr {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}
#[doc = "`MeasureName`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct MeasureName(::std::string::String);
impl ::std::ops::Deref for MeasureName {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<MeasureName> for ::std::string::String {
    fn from(value: MeasureName) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for MeasureName {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"(?!.*__).*^[a-z][a-z0-9_]*[a-z0-9]$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for MeasureName {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MeasureName {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for MeasureName {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`MetricInputMeasure`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged, deny_unknown_fields)]
pub enum MetricInputMeasure {
    String(::std::string::String),
    Object {
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        fill_nulls_with: ::std::option::Option<MetricInputMeasureObjectFillNullsWith>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        filter: ::std::option::Option<::std::string::String>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        join_to_timespine: ::std::option::Option<bool>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        name: ::std::option::Option<::std::string::String>,
    },
}
#[doc = "`MetricInputMeasureObjectFillNullsWith`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum MetricInputMeasureObjectFillNullsWith {
    String(::std::string::String),
    Integer(i64),
}
impl ::std::fmt::Display for MetricInputMeasureObjectFillNullsWith {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::String(x) => x.fmt(f),
            Self::Integer(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<i64> for MetricInputMeasureObjectFillNullsWith {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}
#[doc = "`MetricInputSchema`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged, deny_unknown_fields)]
pub enum MetricInputSchema {
    String(::std::string::String),
    Object {
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        alias: ::std::option::Option<::std::string::String>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        filter: ::std::option::Option<::std::string::String>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        name: ::std::option::Option<::std::string::String>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        offset_to_grain: ::std::option::Option<::std::string::String>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        offset_window: ::std::option::Option<::std::string::String>,
    },
}
#[doc = "`ModelConfigs`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct ModelConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub auto_refresh: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub backup: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub contract: ::std::option::Option<ModelConfigsContract>,
    #[doc = "The column that represents the actual moment the row's event happened. Contrast with, for example, when it was loaded to the warehouse."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub event_time: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub file_format: ::std::option::Option<::std::string::String>,
    #[doc = "Configuration, specific to BigQuery adapter, used to setup authorized views."]
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub grant_access_to: ::std::vec::Vec<AuthorizedViewsItem>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grants: ::std::option::Option<Grants>,
    #[doc = "Configuration specific to BigQuery adapter used to set an expiration delay (in hours) to a table."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub hours_to_expiration: ::std::option::Option<f64>,
    #[doc = "Configuration of the KMS key name, specific to BigQuery adapter."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub kms_key_name: ::std::option::Option<ModelConfigsKmsKeyName>,
    #[doc = "Configuration specific to BigQuery adapter used to add labels and tags to tables/views created by dbt."]
    #[serde(
        default,
        skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
    )]
    pub labels: ::std::collections::HashMap<ModelConfigsLabelsKey, ModelConfigsLabelsValue>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub location: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub materialized: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub on_configuration_change: ::std::option::Option<ModelConfigsOnConfigurationChange>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub on_schema_change: ::std::option::Option<ModelConfigsOnSchemaChange>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub snowflake_warehouse: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub sql_header: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub target_lag: ::std::option::Option<ModelConfigsTargetLag>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub tblproperties: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl ModelConfigs {
    pub fn builder() -> builder::ModelConfigs {
        Default::default()
    }
}
#[doc = "`ModelConfigsContract`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct ModelConfigsContract {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enforced: ::std::option::Option<BooleanOrJinjaString>,
}
impl ModelConfigsContract {
    pub fn builder() -> builder::ModelConfigsContract {
        Default::default()
    }
}
#[doc = "Configuration of the KMS key name, specific to BigQuery adapter."]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ModelConfigsKmsKeyName(::std::string::String);
impl ::std::ops::Deref for ModelConfigsKmsKeyName {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ModelConfigsKmsKeyName> for ::std::string::String {
    fn from(value: ModelConfigsKmsKeyName) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ModelConfigsKmsKeyName {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new(
                    "projects/[a-zA-Z0-9_-]*/locations/[a-zA-Z0-9_-]*/keyRings/.*/cryptoKeys/.*",
                )
                .unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err ("doesn't match pattern \"projects/[a-zA-Z0-9_-]*/locations/[a-zA-Z0-9_-]*/keyRings/.*/cryptoKeys/.*\"" . into ()) ;
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for ModelConfigsKmsKeyName {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ModelConfigsKmsKeyName {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ModelConfigsKmsKeyName {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`ModelConfigsLabelsKey`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ModelConfigsLabelsKey(::std::string::String);
impl ::std::ops::Deref for ModelConfigsLabelsKey {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ModelConfigsLabelsKey> for ::std::string::String {
    fn from(value: ModelConfigsLabelsKey) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ModelConfigsLabelsKey {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("^[a-z][a-z0-9_-]{0,62}$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-z][a-z0-9_-]{0,62}$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for ModelConfigsLabelsKey {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ModelConfigsLabelsKey {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ModelConfigsLabelsKey {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`ModelConfigsLabelsValue`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ModelConfigsLabelsValue(::std::string::String);
impl ::std::ops::Deref for ModelConfigsLabelsValue {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ModelConfigsLabelsValue> for ::std::string::String {
    fn from(value: ModelConfigsLabelsValue) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ModelConfigsLabelsValue {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[a-z0-9_-]{0,63}$").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-z0-9_-]{0,63}$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for ModelConfigsLabelsValue {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ModelConfigsLabelsValue {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ModelConfigsLabelsValue {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`ModelConfigsOnConfigurationChange`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ModelConfigsOnConfigurationChange {
    #[serde(rename = "apply")]
    Apply,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "fail")]
    Fail,
}
impl ::std::fmt::Display for ModelConfigsOnConfigurationChange {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Apply => f.write_str("apply"),
            Self::Continue => f.write_str("continue"),
            Self::Fail => f.write_str("fail"),
        }
    }
}
impl ::std::str::FromStr for ModelConfigsOnConfigurationChange {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "apply" => Ok(Self::Apply),
            "continue" => Ok(Self::Continue),
            "fail" => Ok(Self::Fail),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ModelConfigsOnConfigurationChange {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ModelConfigsOnConfigurationChange {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ModelConfigsOnSchemaChange`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ModelConfigsOnSchemaChange {
    #[serde(rename = "append_new_columns")]
    AppendNewColumns,
    #[serde(rename = "fail")]
    Fail,
    #[serde(rename = "ignore")]
    Ignore,
    #[serde(rename = "sync_all_columns")]
    SyncAllColumns,
}
impl ::std::fmt::Display for ModelConfigsOnSchemaChange {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::AppendNewColumns => f.write_str("append_new_columns"),
            Self::Fail => f.write_str("fail"),
            Self::Ignore => f.write_str("ignore"),
            Self::SyncAllColumns => f.write_str("sync_all_columns"),
        }
    }
}
impl ::std::str::FromStr for ModelConfigsOnSchemaChange {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "append_new_columns" => Ok(Self::AppendNewColumns),
            "fail" => Ok(Self::Fail),
            "ignore" => Ok(Self::Ignore),
            "sync_all_columns" => Ok(Self::SyncAllColumns),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ModelConfigsOnSchemaChange {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ModelConfigsOnSchemaChange {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ModelConfigsTargetLag`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ModelConfigsTargetLag(::std::string::String);
impl ::std::ops::Deref for ModelConfigsTargetLag {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ModelConfigsTargetLag> for ::std::string::String {
    fn from(value: ModelConfigsTargetLag) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ModelConfigsTargetLag {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("^(?:downstream|\\d+\\s*(?:seconds|minutes|hours|days))$")
                    .unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err(
                "doesn't match pattern \"^(?:downstream|\\d+\\s*(?:seconds|minutes|hours|days))$\""
                    .into(),
            );
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for ModelConfigsTargetLag {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ModelConfigsTargetLag {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ModelConfigsTargetLag {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`NonAdditiveDimension`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NonAdditiveDimension {
    pub name: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub window_choice: ::std::option::Option<NonAdditiveDimensionWindowChoice>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub window_groupings: ::std::vec::Vec<::std::string::String>,
}
impl NonAdditiveDimension {
    pub fn builder() -> builder::NonAdditiveDimension {
        Default::default()
    }
}
#[doc = "`NonAdditiveDimensionWindowChoice`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum NonAdditiveDimensionWindowChoice {
    #[serde(rename = "MAX")]
    Max,
    #[serde(rename = "MIN")]
    Min,
}
impl ::std::fmt::Display for NonAdditiveDimensionWindowChoice {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Max => f.write_str("MAX"),
            Self::Min => f.write_str("MIN"),
        }
    }
}
impl ::std::str::FromStr for NonAdditiveDimensionWindowChoice {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "MAX" => Ok(Self::Max),
            "MIN" => Ok(Self::Min),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for NonAdditiveDimensionWindowChoice {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NonAdditiveDimensionWindowChoice {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`NumberOrJinjaString`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum NumberOrJinjaString {
    Variant0(NumberOrJinjaStringVariant0),
    Variant1(f64),
}
impl ::std::fmt::Display for NumberOrJinjaString {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Variant0(x) => x.fmt(f),
            Self::Variant1(x) => x.fmt(f),
        }
    }
}
impl ::std::str::FromStr for NumberOrJinjaString {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if let Ok(v) = value.parse() {
            Ok(Self::Variant0(v))
        } else if let Ok(v) = value.parse() {
            Ok(Self::Variant1(v))
        } else {
            Err("string conversion failed for all variants".into())
        }
    }
}
impl ::std::convert::TryFrom<&str> for NumberOrJinjaString {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NumberOrJinjaString {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::From<NumberOrJinjaStringVariant0> for NumberOrJinjaString {
    fn from(value: NumberOrJinjaStringVariant0) -> Self {
        Self::Variant0(value)
    }
}
impl ::std::convert::From<f64> for NumberOrJinjaString {
    fn from(value: f64) -> Self {
        Self::Variant1(value)
    }
}
#[doc = "`NumberOrJinjaStringVariant0`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct NumberOrJinjaStringVariant0(::std::string::String);
impl ::std::ops::Deref for NumberOrJinjaStringVariant0 {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<NumberOrJinjaStringVariant0> for ::std::string::String {
    fn from(value: NumberOrJinjaStringVariant0) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for NumberOrJinjaStringVariant0 {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("\\{\\{.*\\}\\}").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"\\{\\{.*\\}\\}\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for NumberOrJinjaStringVariant0 {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NumberOrJinjaStringVariant0 {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for NumberOrJinjaStringVariant0 {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "Configurations for the persistence of the dbt documentation."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct PersistDocsConfig {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub columns: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub relation: ::std::option::Option<BooleanOrJinjaString>,
}
impl PersistDocsConfig {
    pub fn builder() -> builder::PersistDocsConfig {
        Default::default()
    }
}
#[doc = "`RatioMetricTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct RatioMetricTypeParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub denominator: ::std::option::Option<MetricInputSchema>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub filter: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub numerator: ::std::option::Option<MetricInputSchema>,
}
impl RatioMetricTypeParams {
    pub fn builder() -> builder::RatioMetricTypeParams {
        Default::default()
    }
}
#[doc = "`RelationshipsTest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct RelationshipsTest {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub relationships: ::std::option::Option<RelationshipsTestRelationships>,
}
impl RelationshipsTest {
    pub fn builder() -> builder::RelationshipsTest {
        Default::default()
    }
}
#[doc = "`RelationshipsTestRelationships`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RelationshipsTestRelationships {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub config: ::std::option::Option<TestConfigs>,
    #[doc = "The foreign key column"]
    pub field: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    pub to: ::std::string::String,
    #[serde(
        rename = "where",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub where_: ::std::option::Option<::std::string::String>,
}
impl RelationshipsTestRelationships {
    pub fn builder() -> builder::RelationshipsTestRelationships {
        Default::default()
    }
}
#[doc = "`SimpleMetricTypeParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct SimpleMetricTypeParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub measure: ::std::option::Option<MetricInputMeasure>,
}
impl SimpleMetricTypeParams {
    pub fn builder() -> builder::SimpleMetricTypeParams {
        Default::default()
    }
}
#[doc = "`StringOrArrayOfStrings`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum StringOrArrayOfStrings {
    String(::std::string::String),
    ArrayOfStrings(ArrayOfStrings),
}
impl ::std::convert::From<ArrayOfStrings> for StringOrArrayOfStrings {
    fn from(value: ArrayOfStrings) -> Self {
        Self::ArrayOfStrings(value)
    }
}
#[doc = "Configurations set here will override configs set in dbt_project.yml."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct TestConfigs {
    #[doc = "Only relevant when `store_failures` is true"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub alias: ::std::option::Option<::std::string::String>,
    #[doc = "Only relevant when `store_failures` is true"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub error_if: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub fail_calc: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub limit: ::std::option::Option<i64>,
    #[doc = "Only relevant when `store_failures` is true"]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub severity: ::std::option::Option<TestConfigsSeverity>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub store_failures: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub warn_if: ::std::option::Option<::std::string::String>,
}
impl TestConfigs {
    pub fn builder() -> builder::TestConfigs {
        Default::default()
    }
}
#[doc = "`TestConfigsSeverity`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TestConfigsSeverity {
    JinjaString(JinjaString),
    String(TestConfigsSeverityString),
}
impl ::std::fmt::Display for TestConfigsSeverity {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::JinjaString(x) => x.fmt(f),
            Self::String(x) => x.fmt(f),
        }
    }
}
impl ::std::str::FromStr for TestConfigsSeverity {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if let Ok(v) = value.parse() {
            Ok(Self::JinjaString(v))
        } else if let Ok(v) = value.parse() {
            Ok(Self::String(v))
        } else {
            Err("string conversion failed for all variants".into())
        }
    }
}
impl ::std::convert::TryFrom<&str> for TestConfigsSeverity {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TestConfigsSeverity {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::From<JinjaString> for TestConfigsSeverity {
    fn from(value: JinjaString) -> Self {
        Self::JinjaString(value)
    }
}
impl ::std::convert::From<TestConfigsSeverityString> for TestConfigsSeverity {
    fn from(value: TestConfigsSeverityString) -> Self {
        Self::String(value)
    }
}
#[doc = "`TestConfigsSeverityString`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum TestConfigsSeverityString {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warn")]
    Warn,
}
impl ::std::fmt::Display for TestConfigsSeverityString {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Error => f.write_str("error"),
            Self::Warn => f.write_str("warn"),
        }
    }
}
impl ::std::str::FromStr for TestConfigsSeverityString {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TestConfigsSeverityString {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TestConfigsSeverityString {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ValidityParams`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct ValidityParams {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub is_end: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub is_start: ::std::option::Option<bool>,
}
impl ValidityParams {
    pub fn builder() -> builder::ValidityParams {
        Default::default()
    }
}
#[doc = " Types for composing complex structures."]
pub mod builder {
    #[derive(Clone, Debug)]
    pub struct AcceptedValuesTest {
        accepted_values: ::std::result::Result<
            ::std::option::Option<super::AcceptedValuesTestAcceptedValues>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for AcceptedValuesTest {
        fn default() -> Self {
            Self {
                accepted_values: Ok(Default::default()),
            }
        }
    }
    impl AcceptedValuesTest {
        pub fn accepted_values<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::AcceptedValuesTestAcceptedValues>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.accepted_values = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for accepted_values: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AcceptedValuesTest> for super::AcceptedValuesTest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AcceptedValuesTest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                accepted_values: value.accepted_values?,
            })
        }
    }
    impl ::std::convert::From<super::AcceptedValuesTest> for AcceptedValuesTest {
        fn from(value: super::AcceptedValuesTest) -> Self {
            Self {
                accepted_values: Ok(value.accepted_values),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AcceptedValuesTestAcceptedValues {
        config:
            ::std::result::Result<::std::option::Option<super::TestConfigs>, ::std::string::String>,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        quote: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        values:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        where_: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for AcceptedValuesTestAcceptedValues {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                name: Ok(Default::default()),
                quote: Ok(Default::default()),
                values: Err("no value supplied for values".to_string()),
                where_: Ok(Default::default()),
            }
        }
    }
    impl AcceptedValuesTestAcceptedValues {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TestConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn quote<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.quote = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for quote: {e}"));
            self
        }
        pub fn values<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.values = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for values: {e}"));
            self
        }
        pub fn where_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.where_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for where_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AcceptedValuesTestAcceptedValues>
        for super::AcceptedValuesTestAcceptedValues
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AcceptedValuesTestAcceptedValues,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                name: value.name?,
                quote: value.quote?,
                values: value.values?,
                where_: value.where_?,
            })
        }
    }
    impl ::std::convert::From<super::AcceptedValuesTestAcceptedValues>
        for AcceptedValuesTestAcceptedValues
    {
        fn from(value: super::AcceptedValuesTestAcceptedValues) -> Self {
            Self {
                config: Ok(value.config),
                name: Ok(value.name),
                quote: Ok(value.quote),
                values: Ok(value.values),
                where_: Ok(value.where_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AggregationTypeParams {
        percentile: ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
        use_approximate_percentile:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        use_discrete_percentile:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
    }
    impl ::std::default::Default for AggregationTypeParams {
        fn default() -> Self {
            Self {
                percentile: Ok(Default::default()),
                use_approximate_percentile: Ok(Default::default()),
                use_discrete_percentile: Ok(Default::default()),
            }
        }
    }
    impl AggregationTypeParams {
        pub fn percentile<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<f64>>,
            T::Error: ::std::fmt::Display,
        {
            self.percentile = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for percentile: {e}"));
            self
        }
        pub fn use_approximate_percentile<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.use_approximate_percentile = value.try_into().map_err(|e| {
                format!("error converting supplied value for use_approximate_percentile: {e}")
            });
            self
        }
        pub fn use_discrete_percentile<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.use_discrete_percentile = value.try_into().map_err(|e| {
                format!("error converting supplied value for use_discrete_percentile: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<AggregationTypeParams> for super::AggregationTypeParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AggregationTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                percentile: value.percentile?,
                use_approximate_percentile: value.use_approximate_percentile?,
                use_discrete_percentile: value.use_discrete_percentile?,
            })
        }
    }
    impl ::std::convert::From<super::AggregationTypeParams> for AggregationTypeParams {
        fn from(value: super::AggregationTypeParams) -> Self {
            Self {
                percentile: Ok(value.percentile),
                use_approximate_percentile: Ok(value.use_approximate_percentile),
                use_discrete_percentile: Ok(value.use_discrete_percentile),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AuthorizedViewsItem {
        database: ::std::result::Result<::std::string::String, ::std::string::String>,
        project: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for AuthorizedViewsItem {
        fn default() -> Self {
            Self {
                database: Err("no value supplied for database".to_string()),
                project: Err("no value supplied for project".to_string()),
            }
        }
    }
    impl AuthorizedViewsItem {
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
            self
        }
        pub fn project<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.project = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for project: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AuthorizedViewsItem> for super::AuthorizedViewsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AuthorizedViewsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                database: value.database?,
                project: value.project?,
            })
        }
    }
    impl ::std::convert::From<super::AuthorizedViewsItem> for AuthorizedViewsItem {
        fn from(value: super::AuthorizedViewsItem) -> Self {
            Self {
                database: Ok(value.database),
                project: Ok(value.project),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ColumnProperties {
        constraints:
            ::std::result::Result<::std::option::Option<super::Constraints>, ::std::string::String>,
        data_tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
        data_type: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        granularity: ::std::result::Result<
            ::std::option::Option<super::ColumnPropertiesGranularity>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        policy_tags:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        quote: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
    }
    impl ::std::default::Default for ColumnProperties {
        fn default() -> Self {
            Self {
                constraints: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                data_type: Ok(Default::default()),
                description: Ok(Default::default()),
                granularity: Ok(Default::default()),
                meta: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                policy_tags: Ok(Default::default()),
                quote: Ok(Default::default()),
                tags: Ok(Default::default()),
                tests: Ok(Default::default()),
            }
        }
    }
    impl ColumnProperties {
        pub fn constraints<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Constraints>>,
            T::Error: ::std::fmt::Display,
        {
            self.constraints = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for constraints: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn data_type<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_type = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_type: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn granularity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ColumnPropertiesGranularity>>,
            T::Error: ::std::fmt::Display,
        {
            self.granularity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for granularity: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn policy_tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.policy_tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for policy_tags: {e}"));
            self
        }
        pub fn quote<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.quote = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for quote: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tests: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ColumnProperties> for super::ColumnProperties {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ColumnProperties,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                constraints: value.constraints?,
                data_tests: value.data_tests?,
                data_type: value.data_type?,
                description: value.description?,
                granularity: value.granularity?,
                meta: value.meta?,
                name: value.name?,
                policy_tags: value.policy_tags?,
                quote: value.quote?,
                tags: value.tags?,
                tests: value.tests?,
            })
        }
    }
    impl ::std::convert::From<super::ColumnProperties> for ColumnProperties {
        fn from(value: super::ColumnProperties) -> Self {
            Self {
                constraints: Ok(value.constraints),
                data_tests: Ok(value.data_tests),
                data_type: Ok(value.data_type),
                description: Ok(value.description),
                granularity: Ok(value.granularity),
                meta: Ok(value.meta),
                name: Ok(value.name),
                policy_tags: Ok(value.policy_tags),
                quote: Ok(value.quote),
                tags: Ok(value.tags),
                tests: Ok(value.tests),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ConstraintsItem {
        columns: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        expression: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        type_: ::std::result::Result<::std::string::String, ::std::string::String>,
        warn_unenforced: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        warn_unsupported: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ConstraintsItem {
        fn default() -> Self {
            Self {
                columns: Ok(Default::default()),
                expression: Ok(Default::default()),
                name: Ok(Default::default()),
                type_: Err("no value supplied for type_".to_string()),
                warn_unenforced: Ok(Default::default()),
                warn_unsupported: Ok(Default::default()),
            }
        }
    }
    impl ConstraintsItem {
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn expression<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.expression = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expression: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
        pub fn warn_unenforced<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.warn_unenforced = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for warn_unenforced: {e}"));
            self
        }
        pub fn warn_unsupported<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.warn_unsupported = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for warn_unsupported: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ConstraintsItem> for super::ConstraintsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ConstraintsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                columns: value.columns?,
                expression: value.expression?,
                name: value.name?,
                type_: value.type_?,
                warn_unenforced: value.warn_unenforced?,
                warn_unsupported: value.warn_unsupported?,
            })
        }
    }
    impl ::std::convert::From<super::ConstraintsItem> for ConstraintsItem {
        fn from(value: super::ConstraintsItem) -> Self {
            Self {
                columns: Ok(value.columns),
                expression: Ok(value.expression),
                name: Ok(value.name),
                type_: Ok(value.type_),
                warn_unenforced: Ok(value.warn_unenforced),
                warn_unsupported: Ok(value.warn_unsupported),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ConversionMetricTypeParams {
        conversion_type_params: ::std::result::Result<
            ::std::option::Option<super::ConversionMetricTypeParamsConversionTypeParams>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ConversionMetricTypeParams {
        fn default() -> Self {
            Self {
                conversion_type_params: Ok(Default::default()),
            }
        }
    }
    impl ConversionMetricTypeParams {
        pub fn conversion_type_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::ConversionMetricTypeParamsConversionTypeParams>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.conversion_type_params = value.try_into().map_err(|e| {
                format!("error converting supplied value for conversion_type_params: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<ConversionMetricTypeParams> for super::ConversionMetricTypeParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ConversionMetricTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                conversion_type_params: value.conversion_type_params?,
            })
        }
    }
    impl ::std::convert::From<super::ConversionMetricTypeParams> for ConversionMetricTypeParams {
        fn from(value: super::ConversionMetricTypeParams) -> Self {
            Self {
                conversion_type_params: Ok(value.conversion_type_params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ConversionMetricTypeParamsConversionTypeParams {
        base_measure: ::std::result::Result<super::MetricInputMeasure, ::std::string::String>,
        calculation: ::std::result::Result<
            ::std::option::Option<super::ConversionMetricTypeParamsConversionTypeParamsCalculation>,
            ::std::string::String,
        >,
        constant_properties: ::std::result::Result<
            ::std::vec::Vec<
                super::ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem,
            >,
            ::std::string::String,
        >,
        conversion_measure: ::std::result::Result<super::MetricInputMeasure, ::std::string::String>,
        entity: ::std::result::Result<::std::string::String, ::std::string::String>,
        window: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ConversionMetricTypeParamsConversionTypeParams {
        fn default() -> Self {
            Self {
                base_measure: Err("no value supplied for base_measure".to_string()),
                calculation: Ok(Default::default()),
                constant_properties: Ok(Default::default()),
                conversion_measure: Err("no value supplied for conversion_measure".to_string()),
                entity: Err("no value supplied for entity".to_string()),
                window: Ok(Default::default()),
            }
        }
    }
    impl ConversionMetricTypeParamsConversionTypeParams {
        pub fn base_measure<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::MetricInputMeasure>,
            T::Error: ::std::fmt::Display,
        {
            self.base_measure = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for base_measure: {e}"));
            self
        }
        pub fn calculation<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    super::ConversionMetricTypeParamsConversionTypeParamsCalculation,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.calculation = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for calculation: {e}"));
            self
        }
        pub fn constant_properties<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::vec::Vec<
                    super::ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.constant_properties = value.try_into().map_err(|e| {
                format!("error converting supplied value for constant_properties: {e}")
            });
            self
        }
        pub fn conversion_measure<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::MetricInputMeasure>,
            T::Error: ::std::fmt::Display,
        {
            self.conversion_measure = value.try_into().map_err(|e| {
                format!("error converting supplied value for conversion_measure: {e}")
            });
            self
        }
        pub fn entity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.entity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for entity: {e}"));
            self
        }
        pub fn window<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.window = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for window: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ConversionMetricTypeParamsConversionTypeParams>
        for super::ConversionMetricTypeParamsConversionTypeParams
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ConversionMetricTypeParamsConversionTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                base_measure: value.base_measure?,
                calculation: value.calculation?,
                constant_properties: value.constant_properties?,
                conversion_measure: value.conversion_measure?,
                entity: value.entity?,
                window: value.window?,
            })
        }
    }
    impl ::std::convert::From<super::ConversionMetricTypeParamsConversionTypeParams>
        for ConversionMetricTypeParamsConversionTypeParams
    {
        fn from(value: super::ConversionMetricTypeParamsConversionTypeParams) -> Self {
            Self {
                base_measure: Ok(value.base_measure),
                calculation: Ok(value.calculation),
                constant_properties: Ok(value.constant_properties),
                conversion_measure: Ok(value.conversion_measure),
                entity: Ok(value.entity),
                window: Ok(value.window),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem {
        base_property: ::std::result::Result<::std::string::String, ::std::string::String>,
        conversion_property: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default
        for ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem
    {
        fn default() -> Self {
            Self {
                base_property: Err("no value supplied for base_property".to_string()),
                conversion_property: Err("no value supplied for conversion_property".to_string()),
            }
        }
    }
    impl ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem {
        pub fn base_property<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.base_property = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for base_property: {e}"));
            self
        }
        pub fn conversion_property<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.conversion_property = value.try_into().map_err(|e| {
                format!("error converting supplied value for conversion_property: {e}")
            });
            self
        }
    }
    impl
        ::std::convert::TryFrom<
            ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem,
        > for super::ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                base_property: value.base_property?,
                conversion_property: value.conversion_property?,
            })
        }
    }
    impl
        ::std::convert::From<
            super::ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem,
        > for ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem
    {
        fn from(
            value: super::ConversionMetricTypeParamsConversionTypeParamsConstantPropertiesItem,
        ) -> Self {
            Self {
                base_property: Ok(value.base_property),
                conversion_property: Ok(value.conversion_property),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CumulativeMetricTypeParams {
        cumulative_type_params: ::std::result::Result<
            ::std::option::Option<super::CumulativeMetricTypeParamsCumulativeTypeParams>,
            ::std::string::String,
        >,
        grain_to_date: ::std::result::Result<
            ::std::option::Option<super::CumulativeMetricTypeParamsGrainToDate>,
            ::std::string::String,
        >,
        measure: ::std::result::Result<
            ::std::option::Option<super::MetricInputMeasure>,
            ::std::string::String,
        >,
        window: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for CumulativeMetricTypeParams {
        fn default() -> Self {
            Self {
                cumulative_type_params: Ok(Default::default()),
                grain_to_date: Ok(Default::default()),
                measure: Ok(Default::default()),
                window: Ok(Default::default()),
            }
        }
    }
    impl CumulativeMetricTypeParams {
        pub fn cumulative_type_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::CumulativeMetricTypeParamsCumulativeTypeParams>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.cumulative_type_params = value.try_into().map_err(|e| {
                format!("error converting supplied value for cumulative_type_params: {e}")
            });
            self
        }
        pub fn grain_to_date<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::CumulativeMetricTypeParamsGrainToDate>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.grain_to_date = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for grain_to_date: {e}"));
            self
        }
        pub fn measure<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::MetricInputMeasure>>,
            T::Error: ::std::fmt::Display,
        {
            self.measure = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for measure: {e}"));
            self
        }
        pub fn window<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.window = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for window: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<CumulativeMetricTypeParams> for super::CumulativeMetricTypeParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CumulativeMetricTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                cumulative_type_params: value.cumulative_type_params?,
                grain_to_date: value.grain_to_date?,
                measure: value.measure?,
                window: value.window?,
            })
        }
    }
    impl ::std::convert::From<super::CumulativeMetricTypeParams> for CumulativeMetricTypeParams {
        fn from(value: super::CumulativeMetricTypeParams) -> Self {
            Self {
                cumulative_type_params: Ok(value.cumulative_type_params),
                grain_to_date: Ok(value.grain_to_date),
                measure: Ok(value.measure),
                window: Ok(value.window),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CumulativeMetricTypeParamsCumulativeTypeParams {
        grain_to_date: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        period_agg: ::std::result::Result<
            ::std::option::Option<super::CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg>,
            ::std::string::String,
        >,
        window: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for CumulativeMetricTypeParamsCumulativeTypeParams {
        fn default() -> Self {
            Self {
                grain_to_date: Ok(Default::default()),
                period_agg: Ok(Default::default()),
                window: Ok(Default::default()),
            }
        }
    }
    impl CumulativeMetricTypeParamsCumulativeTypeParams {
        pub fn grain_to_date<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.grain_to_date = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for grain_to_date: {e}"));
            self
        }
        pub fn period_agg<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    super::CumulativeMetricTypeParamsCumulativeTypeParamsPeriodAgg,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.period_agg = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for period_agg: {e}"));
            self
        }
        pub fn window<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.window = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for window: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<CumulativeMetricTypeParamsCumulativeTypeParams>
        for super::CumulativeMetricTypeParamsCumulativeTypeParams
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CumulativeMetricTypeParamsCumulativeTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                grain_to_date: value.grain_to_date?,
                period_agg: value.period_agg?,
                window: value.window?,
            })
        }
    }
    impl ::std::convert::From<super::CumulativeMetricTypeParamsCumulativeTypeParams>
        for CumulativeMetricTypeParamsCumulativeTypeParams
    {
        fn from(value: super::CumulativeMetricTypeParamsCumulativeTypeParams) -> Self {
            Self {
                grain_to_date: Ok(value.grain_to_date),
                period_agg: Ok(value.period_agg),
                window: Ok(value.window),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DataTests {
        subtype_0: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        subtype_1: ::std::result::Result<
            ::std::option::Option<super::DataTestsSubtype1>,
            ::std::string::String,
        >,
        subtype_2: ::std::result::Result<
            ::std::option::Option<super::RelationshipsTest>,
            ::std::string::String,
        >,
        subtype_3: ::std::result::Result<
            ::std::option::Option<super::AcceptedValuesTest>,
            ::std::string::String,
        >,
        subtype_4: ::std::result::Result<
            ::std::option::Option<
                ::std::collections::HashMap<
                    super::DataTestsSubtype4Key,
                    super::DataTestsSubtype4Value,
                >,
            >,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DataTests {
        fn default() -> Self {
            Self {
                subtype_0: Ok(Default::default()),
                subtype_1: Ok(Default::default()),
                subtype_2: Ok(Default::default()),
                subtype_3: Ok(Default::default()),
                subtype_4: Ok(Default::default()),
            }
        }
    }
    impl DataTests {
        pub fn subtype_0<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_0 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_0: {e}"));
            self
        }
        pub fn subtype_1<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DataTestsSubtype1>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_1 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_1: {e}"));
            self
        }
        pub fn subtype_2<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::RelationshipsTest>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_2 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_2: {e}"));
            self
        }
        pub fn subtype_3<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::AcceptedValuesTest>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_3 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_3: {e}"));
            self
        }
        pub fn subtype_4<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::std::collections::HashMap<
                        super::DataTestsSubtype4Key,
                        super::DataTestsSubtype4Value,
                    >,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_4 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_4: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DataTests> for super::DataTests {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DataTests,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                subtype_0: value.subtype_0?,
                subtype_1: value.subtype_1?,
                subtype_2: value.subtype_2?,
                subtype_3: value.subtype_3?,
                subtype_4: value.subtype_4?,
            })
        }
    }
    impl ::std::convert::From<super::DataTests> for DataTests {
        fn from(value: super::DataTests) -> Self {
            Self {
                subtype_0: Ok(value.subtype_0),
                subtype_1: Ok(value.subtype_1),
                subtype_2: Ok(value.subtype_2),
                subtype_3: Ok(value.subtype_3),
                subtype_4: Ok(value.subtype_4),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DataTestsSubtype4Value {
        config:
            ::std::result::Result<::std::option::Option<super::TestConfigs>, ::std::string::String>,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        where_: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DataTestsSubtype4Value {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                name: Ok(Default::default()),
                where_: Ok(Default::default()),
            }
        }
    }
    impl DataTestsSubtype4Value {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TestConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn where_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.where_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for where_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DataTestsSubtype4Value> for super::DataTestsSubtype4Value {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DataTestsSubtype4Value,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                name: value.name?,
                where_: value.where_?,
            })
        }
    }
    impl ::std::convert::From<super::DataTestsSubtype4Value> for DataTestsSubtype4Value {
        fn from(value: super::DataTestsSubtype4Value) -> Self {
            Self {
                config: Ok(value.config),
                name: Ok(value.name),
                where_: Ok(value.where_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFiles {
        analyses: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesAnalysesItem>,
            ::std::string::String,
        >,
        data_tests: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesDataTestsItem>,
            ::std::string::String,
        >,
        exposures: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesExposuresItem>,
            ::std::string::String,
        >,
        groups: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesGroupsItem>,
            ::std::string::String,
        >,
        macros: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesMacrosItem>,
            ::std::string::String,
        >,
        metrics: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesMetricsItem>,
            ::std::string::String,
        >,
        models: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesModelsItem>,
            ::std::string::String,
        >,
        saved_queries: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesSavedQueriesItem>,
            ::std::string::String,
        >,
        seeds: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesSeedsItem>,
            ::std::string::String,
        >,
        semantic_models: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesSemanticModelsItem>,
            ::std::string::String,
        >,
        snapshots: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesSnapshotsItem>,
            ::std::string::String,
        >,
        sources: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesSourcesItem>,
            ::std::string::String,
        >,
        unit_tests: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesUnitTestsItem>,
            ::std::string::String,
        >,
        version: ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFiles {
        fn default() -> Self {
            Self {
                analyses: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                exposures: Ok(Default::default()),
                groups: Ok(Default::default()),
                macros: Ok(Default::default()),
                metrics: Ok(Default::default()),
                models: Ok(Default::default()),
                saved_queries: Ok(Default::default()),
                seeds: Ok(Default::default()),
                semantic_models: Ok(Default::default()),
                snapshots: Ok(Default::default()),
                sources: Ok(Default::default()),
                unit_tests: Ok(Default::default()),
                version: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFiles {
        pub fn analyses<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesAnalysesItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.analyses = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for analyses: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesDataTestsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn exposures<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesExposuresItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.exposures = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for exposures: {e}"));
            self
        }
        pub fn groups<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesGroupsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.groups = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for groups: {e}"));
            self
        }
        pub fn macros<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesMacrosItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.macros = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for macros: {e}"));
            self
        }
        pub fn metrics<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesMetricsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.metrics = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metrics: {e}"));
            self
        }
        pub fn models<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesModelsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.models = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for models: {e}"));
            self
        }
        pub fn saved_queries<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesSavedQueriesItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.saved_queries = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for saved_queries: {e}"));
            self
        }
        pub fn seeds<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesSeedsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.seeds = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for seeds: {e}"));
            self
        }
        pub fn semantic_models<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesSemanticModelsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.semantic_models = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for semantic_models: {e}"));
            self
        }
        pub fn snapshots<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesSnapshotsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.snapshots = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for snapshots: {e}"));
            self
        }
        pub fn sources<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesSourcesItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.sources = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for sources: {e}"));
            self
        }
        pub fn unit_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesUnitTestsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.unit_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for unit_tests: {e}"));
            self
        }
        pub fn version<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<f64>>,
            T::Error: ::std::fmt::Display,
        {
            self.version = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for version: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFiles> for super::DbtYmlFiles {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFiles,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                analyses: value.analyses?,
                data_tests: value.data_tests?,
                exposures: value.exposures?,
                groups: value.groups?,
                macros: value.macros?,
                metrics: value.metrics?,
                models: value.models?,
                saved_queries: value.saved_queries?,
                seeds: value.seeds?,
                semantic_models: value.semantic_models?,
                snapshots: value.snapshots?,
                sources: value.sources?,
                unit_tests: value.unit_tests?,
                version: value.version?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFiles> for DbtYmlFiles {
        fn from(value: super::DbtYmlFiles) -> Self {
            Self {
                analyses: Ok(value.analyses),
                data_tests: Ok(value.data_tests),
                exposures: Ok(value.exposures),
                groups: Ok(value.groups),
                macros: Ok(value.macros),
                metrics: Ok(value.metrics),
                models: Ok(value.models),
                saved_queries: Ok(value.saved_queries),
                seeds: Ok(value.seeds),
                semantic_models: Ok(value.semantic_models),
                snapshots: Ok(value.snapshots),
                sources: Ok(value.sources),
                unit_tests: Ok(value.unit_tests),
                version: Ok(value.version),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesAnalysesItem {
        columns: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesAnalysesItemColumnsItem>,
            ::std::string::String,
        >,
        config: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesAnalysesItemConfig>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesAnalysesItem {
        fn default() -> Self {
            Self {
                columns: Ok(Default::default()),
                config: Ok(Default::default()),
                description: Ok(Default::default()),
                docs: Ok(Default::default()),
                group: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
            }
        }
    }
    impl DbtYmlFilesAnalysesItem {
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesAnalysesItemColumnsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtYmlFilesAnalysesItemConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn docs<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DocsConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.docs = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for docs: {e}"));
            self
        }
        pub fn group<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Group>>,
            T::Error: ::std::fmt::Display,
        {
            self.group = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for group: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesAnalysesItem> for super::DbtYmlFilesAnalysesItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesAnalysesItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                columns: value.columns?,
                config: value.config?,
                description: value.description?,
                docs: value.docs?,
                group: value.group?,
                name: value.name?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesAnalysesItem> for DbtYmlFilesAnalysesItem {
        fn from(value: super::DbtYmlFilesAnalysesItem) -> Self {
            Self {
                columns: Ok(value.columns),
                config: Ok(value.config),
                description: Ok(value.description),
                docs: Ok(value.docs),
                group: Ok(value.group),
                name: Ok(value.name),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesAnalysesItemColumnsItem {
        data_type: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesAnalysesItemColumnsItem {
        fn default() -> Self {
            Self {
                data_type: Ok(Default::default()),
                description: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
            }
        }
    }
    impl DbtYmlFilesAnalysesItemColumnsItem {
        pub fn data_type<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_type = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_type: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesAnalysesItemColumnsItem>
        for super::DbtYmlFilesAnalysesItemColumnsItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesAnalysesItemColumnsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                data_type: value.data_type?,
                description: value.description?,
                name: value.name?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesAnalysesItemColumnsItem>
        for DbtYmlFilesAnalysesItemColumnsItem
    {
        fn from(value: super::DbtYmlFilesAnalysesItemColumnsItem) -> Self {
            Self {
                data_type: Ok(value.data_type),
                description: Ok(value.description),
                name: Ok(value.name),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesAnalysesItemConfig {
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesAnalysesItemConfig {
        fn default() -> Self {
            Self {
                tags: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesAnalysesItemConfig {
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesAnalysesItemConfig>
        for super::DbtYmlFilesAnalysesItemConfig
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesAnalysesItemConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { tags: value.tags? })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesAnalysesItemConfig> for DbtYmlFilesAnalysesItemConfig {
        fn from(value: super::DbtYmlFilesAnalysesItemConfig) -> Self {
            Self {
                tags: Ok(value.tags),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesDataTestsItem {
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesDataTestsItem {
        fn default() -> Self {
            Self {
                description: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
            }
        }
    }
    impl DbtYmlFilesDataTestsItem {
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesDataTestsItem> for super::DbtYmlFilesDataTestsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesDataTestsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                description: value.description?,
                name: value.name?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesDataTestsItem> for DbtYmlFilesDataTestsItem {
        fn from(value: super::DbtYmlFilesDataTestsItem) -> Self {
            Self {
                description: Ok(value.description),
                name: Ok(value.name),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesExposuresItem {
        depends_on:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        label: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        maturity: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesExposuresItemMaturity>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        owner: ::std::result::Result<super::DbtYmlFilesExposuresItemOwner, ::std::string::String>,
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        type_: ::std::result::Result<super::DbtYmlFilesExposuresItemType, ::std::string::String>,
        url: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesExposuresItem {
        fn default() -> Self {
            Self {
                depends_on: Err("no value supplied for depends_on".to_string()),
                description: Ok(Default::default()),
                label: Ok(Default::default()),
                maturity: Ok(Default::default()),
                meta: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                owner: Err("no value supplied for owner".to_string()),
                tags: Ok(Default::default()),
                type_: Err("no value supplied for type_".to_string()),
                url: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesExposuresItem {
        pub fn depends_on<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.depends_on = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for depends_on: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn label<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.label = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for label: {e}"));
            self
        }
        pub fn maturity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesExposuresItemMaturity>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.maturity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for maturity: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn owner<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesExposuresItemOwner>,
            T::Error: ::std::fmt::Display,
        {
            self.owner = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for owner: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesExposuresItemType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
        pub fn url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for url: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesExposuresItem> for super::DbtYmlFilesExposuresItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesExposuresItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                depends_on: value.depends_on?,
                description: value.description?,
                label: value.label?,
                maturity: value.maturity?,
                meta: value.meta?,
                name: value.name?,
                owner: value.owner?,
                tags: value.tags?,
                type_: value.type_?,
                url: value.url?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesExposuresItem> for DbtYmlFilesExposuresItem {
        fn from(value: super::DbtYmlFilesExposuresItem) -> Self {
            Self {
                depends_on: Ok(value.depends_on),
                description: Ok(value.description),
                label: Ok(value.label),
                maturity: Ok(value.maturity),
                meta: Ok(value.meta),
                name: Ok(value.name),
                owner: Ok(value.owner),
                tags: Ok(value.tags),
                type_: Ok(value.type_),
                url: Ok(value.url),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesGroupsItem {
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        owner: ::std::result::Result<super::DbtYmlFilesGroupsItemOwner, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesGroupsItem {
        fn default() -> Self {
            Self {
                name: Err("no value supplied for name".to_string()),
                owner: Err("no value supplied for owner".to_string()),
            }
        }
    }
    impl DbtYmlFilesGroupsItem {
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn owner<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesGroupsItemOwner>,
            T::Error: ::std::fmt::Display,
        {
            self.owner = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for owner: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesGroupsItem> for super::DbtYmlFilesGroupsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesGroupsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                name: value.name?,
                owner: value.owner?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesGroupsItem> for DbtYmlFilesGroupsItem {
        fn from(value: super::DbtYmlFilesGroupsItem) -> Self {
            Self {
                name: Ok(value.name),
                owner: Ok(value.owner),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesGroupsItemOwner {
        email: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesGroupsItemOwner {
        fn default() -> Self {
            Self {
                email: Ok(Default::default()),
                name: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesGroupsItemOwner {
        pub fn email<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.email = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for email: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesGroupsItemOwner> for super::DbtYmlFilesGroupsItemOwner {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesGroupsItemOwner,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                email: value.email?,
                name: value.name?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesGroupsItemOwner> for DbtYmlFilesGroupsItemOwner {
        fn from(value: super::DbtYmlFilesGroupsItemOwner) -> Self {
            Self {
                email: Ok(value.email),
                name: Ok(value.name),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesMacrosItem {
        arguments: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesMacrosItemArgumentsItem>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesMacrosItem {
        fn default() -> Self {
            Self {
                arguments: Ok(Default::default()),
                description: Ok(Default::default()),
                docs: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
            }
        }
    }
    impl DbtYmlFilesMacrosItem {
        pub fn arguments<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesMacrosItemArgumentsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.arguments = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for arguments: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn docs<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DocsConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.docs = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for docs: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesMacrosItem> for super::DbtYmlFilesMacrosItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesMacrosItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                arguments: value.arguments?,
                description: value.description?,
                docs: value.docs?,
                name: value.name?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesMacrosItem> for DbtYmlFilesMacrosItem {
        fn from(value: super::DbtYmlFilesMacrosItem) -> Self {
            Self {
                arguments: Ok(value.arguments),
                description: Ok(value.description),
                docs: Ok(value.docs),
                name: Ok(value.name),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesMacrosItemArgumentsItem {
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        type_: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesMacrosItemArgumentsItem {
        fn default() -> Self {
            Self {
                description: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                type_: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesMacrosItemArgumentsItem {
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesMacrosItemArgumentsItem>
        for super::DbtYmlFilesMacrosItemArgumentsItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesMacrosItemArgumentsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                description: value.description?,
                name: value.name?,
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesMacrosItemArgumentsItem>
        for DbtYmlFilesMacrosItemArgumentsItem
    {
        fn from(value: super::DbtYmlFilesMacrosItemArgumentsItem) -> Self {
            Self {
                description: Ok(value.description),
                name: Ok(value.name),
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesMetricsItem {
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        filter: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        label: ::std::result::Result<::std::string::String, ::std::string::String>,
        name: ::std::result::Result<super::DbtYmlFilesMetricsItemName, ::std::string::String>,
        time_granularity: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        type_: ::std::result::Result<super::DbtYmlFilesMetricsItemType, ::std::string::String>,
        type_params: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesMetricsItem {
        fn default() -> Self {
            Self {
                description: Ok(Default::default()),
                filter: Ok(Default::default()),
                group: Ok(Default::default()),
                label: Err("no value supplied for label".to_string()),
                name: Err("no value supplied for name".to_string()),
                time_granularity: Ok(Default::default()),
                type_: Err("no value supplied for type_".to_string()),
                type_params: Err("no value supplied for type_params".to_string()),
            }
        }
    }
    impl DbtYmlFilesMetricsItem {
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn filter<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.filter = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for filter: {e}"));
            self
        }
        pub fn group<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Group>>,
            T::Error: ::std::fmt::Display,
        {
            self.group = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for group: {e}"));
            self
        }
        pub fn label<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.label = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for label: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesMetricsItemName>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn time_granularity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.time_granularity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for time_granularity: {e}"));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesMetricsItemType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
        pub fn type_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.type_params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_params: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesMetricsItem> for super::DbtYmlFilesMetricsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesMetricsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                description: value.description?,
                filter: value.filter?,
                group: value.group?,
                label: value.label?,
                name: value.name?,
                time_granularity: value.time_granularity?,
                type_: value.type_?,
                type_params: value.type_params?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesMetricsItem> for DbtYmlFilesMetricsItem {
        fn from(value: super::DbtYmlFilesMetricsItem) -> Self {
            Self {
                description: Ok(value.description),
                filter: Ok(value.filter),
                group: Ok(value.group),
                label: Ok(value.label),
                name: Ok(value.name),
                time_granularity: Ok(value.time_granularity),
                type_: Ok(value.type_),
                type_params: Ok(value.type_params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesModelsItem {
        access: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesModelsItemAccess>,
            ::std::string::String,
        >,
        columns:
            ::std::result::Result<::std::vec::Vec<super::ColumnProperties>, ::std::string::String>,
        config: ::std::result::Result<
            ::std::option::Option<super::ModelConfigs>,
            ::std::string::String,
        >,
        constraints:
            ::std::result::Result<::std::option::Option<super::Constraints>, ::std::string::String>,
        data_tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
        deprecation_date: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        latest_version: ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
        time_spine: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesModelsItemTimeSpine>,
            ::std::string::String,
        >,
        versions: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesModelsItemVersionsItem>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesModelsItem {
        fn default() -> Self {
            Self {
                access: Ok(Default::default()),
                columns: Ok(Default::default()),
                config: Ok(Default::default()),
                constraints: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                deprecation_date: Ok(Default::default()),
                description: Ok(Default::default()),
                docs: Ok(Default::default()),
                group: Ok(Default::default()),
                latest_version: Ok(Default::default()),
                meta: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                tests: Ok(Default::default()),
                time_spine: Ok(Default::default()),
                versions: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesModelsItem {
        pub fn access<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtYmlFilesModelsItemAccess>>,
            T::Error: ::std::fmt::Display,
        {
            self.access = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for access: {e}"));
            self
        }
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::ColumnProperties>>,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ModelConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn constraints<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Constraints>>,
            T::Error: ::std::fmt::Display,
        {
            self.constraints = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for constraints: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn deprecation_date<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.deprecation_date = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for deprecation_date: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn docs<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DocsConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.docs = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for docs: {e}"));
            self
        }
        pub fn group<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Group>>,
            T::Error: ::std::fmt::Display,
        {
            self.group = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for group: {e}"));
            self
        }
        pub fn latest_version<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<f64>>,
            T::Error: ::std::fmt::Display,
        {
            self.latest_version = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for latest_version: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tests: {e}"));
            self
        }
        pub fn time_spine<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesModelsItemTimeSpine>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.time_spine = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for time_spine: {e}"));
            self
        }
        pub fn versions<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesModelsItemVersionsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.versions = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for versions: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesModelsItem> for super::DbtYmlFilesModelsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesModelsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                access: value.access?,
                columns: value.columns?,
                config: value.config?,
                constraints: value.constraints?,
                data_tests: value.data_tests?,
                deprecation_date: value.deprecation_date?,
                description: value.description?,
                docs: value.docs?,
                group: value.group?,
                latest_version: value.latest_version?,
                meta: value.meta?,
                name: value.name?,
                tests: value.tests?,
                time_spine: value.time_spine?,
                versions: value.versions?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesModelsItem> for DbtYmlFilesModelsItem {
        fn from(value: super::DbtYmlFilesModelsItem) -> Self {
            Self {
                access: Ok(value.access),
                columns: Ok(value.columns),
                config: Ok(value.config),
                constraints: Ok(value.constraints),
                data_tests: Ok(value.data_tests),
                deprecation_date: Ok(value.deprecation_date),
                description: Ok(value.description),
                docs: Ok(value.docs),
                group: Ok(value.group),
                latest_version: Ok(value.latest_version),
                meta: Ok(value.meta),
                name: Ok(value.name),
                tests: Ok(value.tests),
                time_spine: Ok(value.time_spine),
                versions: Ok(value.versions),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesModelsItemTimeSpine {
        standard_granularity_column:
            ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesModelsItemTimeSpine {
        fn default() -> Self {
            Self {
                standard_granularity_column: Err(
                    "no value supplied for standard_granularity_column".to_string(),
                ),
            }
        }
    }
    impl DbtYmlFilesModelsItemTimeSpine {
        pub fn standard_granularity_column<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.standard_granularity_column = value.try_into().map_err(|e| {
                format!("error converting supplied value for standard_granularity_column: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesModelsItemTimeSpine>
        for super::DbtYmlFilesModelsItemTimeSpine
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesModelsItemTimeSpine,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                standard_granularity_column: value.standard_granularity_column?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesModelsItemTimeSpine>
        for DbtYmlFilesModelsItemTimeSpine
    {
        fn from(value: super::DbtYmlFilesModelsItemTimeSpine) -> Self {
            Self {
                standard_granularity_column: Ok(value.standard_granularity_column),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesModelsItemVersionsItem {
        columns: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesModelsItemVersionsItemColumnsItem>,
            ::std::string::String,
        >,
        config: ::std::result::Result<
            ::std::option::Option<super::ModelConfigs>,
            ::std::string::String,
        >,
        v: ::std::result::Result<f64, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesModelsItemVersionsItem {
        fn default() -> Self {
            Self {
                columns: Ok(Default::default()),
                config: Ok(Default::default()),
                v: Err("no value supplied for v".to_string()),
            }
        }
    }
    impl DbtYmlFilesModelsItemVersionsItem {
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::vec::Vec<super::DbtYmlFilesModelsItemVersionsItemColumnsItem>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ModelConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn v<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<f64>,
            T::Error: ::std::fmt::Display,
        {
            self.v = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for v: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesModelsItemVersionsItem>
        for super::DbtYmlFilesModelsItemVersionsItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesModelsItemVersionsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                columns: value.columns?,
                config: value.config?,
                v: value.v?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesModelsItemVersionsItem>
        for DbtYmlFilesModelsItemVersionsItem
    {
        fn from(value: super::DbtYmlFilesModelsItemVersionsItem) -> Self {
            Self {
                columns: Ok(value.columns),
                config: Ok(value.config),
                v: Ok(value.v),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesModelsItemVersionsItemColumnsItem {
        subtype_0: ::std::result::Result<
            ::std::option::Option<super::IncludeExclude>,
            ::std::string::String,
        >,
        subtype_1: ::std::result::Result<
            ::std::option::Option<super::ColumnProperties>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesModelsItemVersionsItemColumnsItem {
        fn default() -> Self {
            Self {
                subtype_0: Ok(Default::default()),
                subtype_1: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesModelsItemVersionsItemColumnsItem {
        pub fn subtype_0<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::IncludeExclude>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_0 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_0: {e}"));
            self
        }
        pub fn subtype_1<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ColumnProperties>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_1 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_1: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesModelsItemVersionsItemColumnsItem>
        for super::DbtYmlFilesModelsItemVersionsItemColumnsItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesModelsItemVersionsItemColumnsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                subtype_0: value.subtype_0?,
                subtype_1: value.subtype_1?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesModelsItemVersionsItemColumnsItem>
        for DbtYmlFilesModelsItemVersionsItemColumnsItem
    {
        fn from(value: super::DbtYmlFilesModelsItemVersionsItemColumnsItem) -> Self {
            Self {
                subtype_0: Ok(value.subtype_0),
                subtype_1: Ok(value.subtype_1),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSavedQueriesItem {
        config: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSavedQueriesItemConfig>,
            ::std::string::String,
        >,
        description: ::std::result::Result<::std::string::String, ::std::string::String>,
        exports: ::std::result::Result<::std::vec::Vec<super::Export>, ::std::string::String>,
        label: ::std::result::Result<::std::string::String, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        query_params: ::std::result::Result<
            super::DbtYmlFilesSavedQueriesItemQueryParams,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSavedQueriesItemTags>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSavedQueriesItem {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                description: Err("no value supplied for description".to_string()),
                exports: Ok(Default::default()),
                label: Err("no value supplied for label".to_string()),
                name: Err("no value supplied for name".to_string()),
                query_params: Err("no value supplied for query_params".to_string()),
                tags: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSavedQueriesItem {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSavedQueriesItemConfig>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn exports<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Export>>,
            T::Error: ::std::fmt::Display,
        {
            self.exports = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for exports: {e}"));
            self
        }
        pub fn label<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.label = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for label: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn query_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesSavedQueriesItemQueryParams>,
            T::Error: ::std::fmt::Display,
        {
            self.query_params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for query_params: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSavedQueriesItemTags>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSavedQueriesItem> for super::DbtYmlFilesSavedQueriesItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSavedQueriesItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                description: value.description?,
                exports: value.exports?,
                label: value.label?,
                name: value.name?,
                query_params: value.query_params?,
                tags: value.tags?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSavedQueriesItem> for DbtYmlFilesSavedQueriesItem {
        fn from(value: super::DbtYmlFilesSavedQueriesItem) -> Self {
            Self {
                config: Ok(value.config),
                description: Ok(value.description),
                exports: Ok(value.exports),
                label: Ok(value.label),
                name: Ok(value.name),
                query_params: Ok(value.query_params),
                tags: Ok(value.tags),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSavedQueriesItemConfig {
        cache: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSavedQueriesItemConfigCache>,
            ::std::string::String,
        >,
        enabled: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        export_as: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSavedQueriesItemConfigExportAs>,
            ::std::string::String,
        >,
        group: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        schema: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSavedQueriesItemConfig {
        fn default() -> Self {
            Self {
                cache: Ok(Default::default()),
                enabled: Ok(Default::default()),
                export_as: Ok(Default::default()),
                group: Ok(Default::default()),
                meta: Ok(Default::default()),
                schema: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSavedQueriesItemConfig {
        pub fn cache<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSavedQueriesItemConfigCache>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.cache = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for cache: {e}"));
            self
        }
        pub fn enabled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.enabled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enabled: {e}"));
            self
        }
        pub fn export_as<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSavedQueriesItemConfigExportAs>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.export_as = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for export_as: {e}"));
            self
        }
        pub fn group<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.group = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for group: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSavedQueriesItemConfig>
        for super::DbtYmlFilesSavedQueriesItemConfig
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSavedQueriesItemConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                cache: value.cache?,
                enabled: value.enabled?,
                export_as: value.export_as?,
                group: value.group?,
                meta: value.meta?,
                schema: value.schema?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSavedQueriesItemConfig>
        for DbtYmlFilesSavedQueriesItemConfig
    {
        fn from(value: super::DbtYmlFilesSavedQueriesItemConfig) -> Self {
            Self {
                cache: Ok(value.cache),
                enabled: Ok(value.enabled),
                export_as: Ok(value.export_as),
                group: Ok(value.group),
                meta: Ok(value.meta),
                schema: Ok(value.schema),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSavedQueriesItemConfigCache {
        enabled: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesSavedQueriesItemConfigCache {
        fn default() -> Self {
            Self {
                enabled: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSavedQueriesItemConfigCache {
        pub fn enabled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.enabled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enabled: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSavedQueriesItemConfigCache>
        for super::DbtYmlFilesSavedQueriesItemConfigCache
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSavedQueriesItemConfigCache,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                enabled: value.enabled?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSavedQueriesItemConfigCache>
        for DbtYmlFilesSavedQueriesItemConfigCache
    {
        fn from(value: super::DbtYmlFilesSavedQueriesItemConfigCache) -> Self {
            Self {
                enabled: Ok(value.enabled),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSavedQueriesItemQueryParams {
        dimensions: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        metrics: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        where_: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSavedQueriesItemQueryParams {
        fn default() -> Self {
            Self {
                dimensions: Ok(Default::default()),
                metrics: Ok(Default::default()),
                where_: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSavedQueriesItemQueryParams {
        pub fn dimensions<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.dimensions = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for dimensions: {e}"));
            self
        }
        pub fn metrics<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.metrics = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metrics: {e}"));
            self
        }
        pub fn where_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.where_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for where_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSavedQueriesItemQueryParams>
        for super::DbtYmlFilesSavedQueriesItemQueryParams
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSavedQueriesItemQueryParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                dimensions: value.dimensions?,
                metrics: value.metrics?,
                where_: value.where_?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSavedQueriesItemQueryParams>
        for DbtYmlFilesSavedQueriesItemQueryParams
    {
        fn from(value: super::DbtYmlFilesSavedQueriesItemQueryParams) -> Self {
            Self {
                dimensions: Ok(value.dimensions),
                metrics: Ok(value.metrics),
                where_: Ok(value.where_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSeedsItem {
        columns:
            ::std::result::Result<::std::vec::Vec<super::ColumnProperties>, ::std::string::String>,
        config: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSeedsItemConfig>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesSeedsItem {
        fn default() -> Self {
            Self {
                columns: Ok(Default::default()),
                config: Ok(Default::default()),
                description: Ok(Default::default()),
                docs: Ok(Default::default()),
                group: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                tests: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSeedsItem {
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::ColumnProperties>>,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtYmlFilesSeedsItemConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn docs<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DocsConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.docs = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for docs: {e}"));
            self
        }
        pub fn group<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Group>>,
            T::Error: ::std::fmt::Display,
        {
            self.group = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for group: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tests: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSeedsItem> for super::DbtYmlFilesSeedsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSeedsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                columns: value.columns?,
                config: value.config?,
                description: value.description?,
                docs: value.docs?,
                group: value.group?,
                name: value.name?,
                tests: value.tests?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSeedsItem> for DbtYmlFilesSeedsItem {
        fn from(value: super::DbtYmlFilesSeedsItem) -> Self {
            Self {
                columns: Ok(value.columns),
                config: Ok(value.config),
                description: Ok(value.description),
                docs: Ok(value.docs),
                group: Ok(value.group),
                name: Ok(value.name),
                tests: Ok(value.tests),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSeedsItemConfig {
        column_types: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        copy_grants: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        data_tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
        database: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        grants: ::std::result::Result<::std::option::Option<super::Grants>, ::std::string::String>,
        quote_columns: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        schema: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSeedsItemConfig {
        fn default() -> Self {
            Self {
                column_types: Ok(Default::default()),
                copy_grants: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                database: Ok(Default::default()),
                enabled: Ok(Default::default()),
                grants: Ok(Default::default()),
                quote_columns: Ok(Default::default()),
                schema: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSeedsItemConfig {
        pub fn column_types<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.column_types = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for column_types: {e}"));
            self
        }
        pub fn copy_grants<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.copy_grants = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for copy_grants: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
            self
        }
        pub fn enabled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.enabled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enabled: {e}"));
            self
        }
        pub fn grants<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Grants>>,
            T::Error: ::std::fmt::Display,
        {
            self.grants = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for grants: {e}"));
            self
        }
        pub fn quote_columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.quote_columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for quote_columns: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSeedsItemConfig> for super::DbtYmlFilesSeedsItemConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSeedsItemConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                column_types: value.column_types?,
                copy_grants: value.copy_grants?,
                data_tests: value.data_tests?,
                database: value.database?,
                enabled: value.enabled?,
                grants: value.grants?,
                quote_columns: value.quote_columns?,
                schema: value.schema?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSeedsItemConfig> for DbtYmlFilesSeedsItemConfig {
        fn from(value: super::DbtYmlFilesSeedsItemConfig) -> Self {
            Self {
                column_types: Ok(value.column_types),
                copy_grants: Ok(value.copy_grants),
                data_tests: Ok(value.data_tests),
                database: Ok(value.database),
                enabled: Ok(value.enabled),
                grants: Ok(value.grants),
                quote_columns: Ok(value.quote_columns),
                schema: Ok(value.schema),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSemanticModelsItem {
        defaults: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSemanticModelsItemDefaults>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        dimensions: ::std::result::Result<::std::vec::Vec<super::Dimension>, ::std::string::String>,
        entities: ::std::result::Result<::std::vec::Vec<super::Entity>, ::std::string::String>,
        measures: ::std::result::Result<::std::vec::Vec<super::Measure>, ::std::string::String>,
        model: ::std::result::Result<::std::string::String, ::std::string::String>,
        name:
            ::std::result::Result<super::DbtYmlFilesSemanticModelsItemName, ::std::string::String>,
        primary_entity: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSemanticModelsItem {
        fn default() -> Self {
            Self {
                defaults: Ok(Default::default()),
                description: Ok(Default::default()),
                dimensions: Ok(Default::default()),
                entities: Ok(Default::default()),
                measures: Ok(Default::default()),
                model: Err("no value supplied for model".to_string()),
                name: Err("no value supplied for name".to_string()),
                primary_entity: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSemanticModelsItem {
        pub fn defaults<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSemanticModelsItemDefaults>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.defaults = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for defaults: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn dimensions<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Dimension>>,
            T::Error: ::std::fmt::Display,
        {
            self.dimensions = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for dimensions: {e}"));
            self
        }
        pub fn entities<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Entity>>,
            T::Error: ::std::fmt::Display,
        {
            self.entities = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for entities: {e}"));
            self
        }
        pub fn measures<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Measure>>,
            T::Error: ::std::fmt::Display,
        {
            self.measures = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for measures: {e}"));
            self
        }
        pub fn model<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.model = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for model: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesSemanticModelsItemName>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn primary_entity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.primary_entity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for primary_entity: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSemanticModelsItem>
        for super::DbtYmlFilesSemanticModelsItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSemanticModelsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                defaults: value.defaults?,
                description: value.description?,
                dimensions: value.dimensions?,
                entities: value.entities?,
                measures: value.measures?,
                model: value.model?,
                name: value.name?,
                primary_entity: value.primary_entity?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSemanticModelsItem> for DbtYmlFilesSemanticModelsItem {
        fn from(value: super::DbtYmlFilesSemanticModelsItem) -> Self {
            Self {
                defaults: Ok(value.defaults),
                description: Ok(value.description),
                dimensions: Ok(value.dimensions),
                entities: Ok(value.entities),
                measures: Ok(value.measures),
                model: Ok(value.model),
                name: Ok(value.name),
                primary_entity: Ok(value.primary_entity),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSemanticModelsItemDefaults {
        agg_time_dimension: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSemanticModelsItemDefaults {
        fn default() -> Self {
            Self {
                agg_time_dimension: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSemanticModelsItemDefaults {
        pub fn agg_time_dimension<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.agg_time_dimension = value.try_into().map_err(|e| {
                format!("error converting supplied value for agg_time_dimension: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSemanticModelsItemDefaults>
        for super::DbtYmlFilesSemanticModelsItemDefaults
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSemanticModelsItemDefaults,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                agg_time_dimension: value.agg_time_dimension?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSemanticModelsItemDefaults>
        for DbtYmlFilesSemanticModelsItemDefaults
    {
        fn from(value: super::DbtYmlFilesSemanticModelsItemDefaults) -> Self {
            Self {
                agg_time_dimension: Ok(value.agg_time_dimension),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSnapshotsItem {
        columns:
            ::std::result::Result<::std::vec::Vec<super::ColumnProperties>, ::std::string::String>,
        config: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSnapshotsItemConfig>,
            ::std::string::String,
        >,
        data_tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        relation: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesSnapshotsItem {
        fn default() -> Self {
            Self {
                columns: Ok(Default::default()),
                config: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                description: Ok(Default::default()),
                docs: Ok(Default::default()),
                group: Ok(Default::default()),
                meta: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                relation: Ok(Default::default()),
                tests: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSnapshotsItem {
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::ColumnProperties>>,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSnapshotsItemConfig>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn docs<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DocsConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.docs = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for docs: {e}"));
            self
        }
        pub fn group<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Group>>,
            T::Error: ::std::fmt::Display,
        {
            self.group = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for group: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn relation<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.relation = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for relation: {e}"));
            self
        }
        pub fn tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tests: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSnapshotsItem> for super::DbtYmlFilesSnapshotsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSnapshotsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                columns: value.columns?,
                config: value.config?,
                data_tests: value.data_tests?,
                description: value.description?,
                docs: value.docs?,
                group: value.group?,
                meta: value.meta?,
                name: value.name?,
                relation: value.relation?,
                tests: value.tests?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSnapshotsItem> for DbtYmlFilesSnapshotsItem {
        fn from(value: super::DbtYmlFilesSnapshotsItem) -> Self {
            Self {
                columns: Ok(value.columns),
                config: Ok(value.config),
                data_tests: Ok(value.data_tests),
                description: Ok(value.description),
                docs: Ok(value.docs),
                group: Ok(value.group),
                meta: Ok(value.meta),
                name: Ok(value.name),
                relation: Ok(value.relation),
                tests: Ok(value.tests),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSnapshotsItemConfig {
        alias: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        check_cols: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        grants: ::std::result::Result<::std::option::Option<super::Grants>, ::std::string::String>,
        persist_docs: ::std::result::Result<
            ::std::option::Option<super::PersistDocsConfig>,
            ::std::string::String,
        >,
        post_hook: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        pre_hook: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        quote_columns: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        strategy: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        target_database: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        target_schema: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        unique_key: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        updated_at: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSnapshotsItemConfig {
        fn default() -> Self {
            Self {
                alias: Ok(Default::default()),
                check_cols: Ok(Default::default()),
                enabled: Ok(Default::default()),
                grants: Ok(Default::default()),
                persist_docs: Ok(Default::default()),
                post_hook: Ok(Default::default()),
                pre_hook: Ok(Default::default()),
                quote_columns: Ok(Default::default()),
                strategy: Ok(Default::default()),
                tags: Ok(Default::default()),
                target_database: Ok(Default::default()),
                target_schema: Ok(Default::default()),
                unique_key: Ok(Default::default()),
                updated_at: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSnapshotsItemConfig {
        pub fn alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.alias = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for alias: {e}"));
            self
        }
        pub fn check_cols<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.check_cols = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for check_cols: {e}"));
            self
        }
        pub fn enabled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.enabled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enabled: {e}"));
            self
        }
        pub fn grants<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Grants>>,
            T::Error: ::std::fmt::Display,
        {
            self.grants = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for grants: {e}"));
            self
        }
        pub fn persist_docs<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::PersistDocsConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.persist_docs = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for persist_docs: {e}"));
            self
        }
        pub fn post_hook<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.post_hook = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for post_hook: {e}"));
            self
        }
        pub fn pre_hook<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.pre_hook = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for pre_hook: {e}"));
            self
        }
        pub fn quote_columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.quote_columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for quote_columns: {e}"));
            self
        }
        pub fn strategy<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.strategy = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for strategy: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn target_database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.target_database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for target_database: {e}"));
            self
        }
        pub fn target_schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.target_schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for target_schema: {e}"));
            self
        }
        pub fn unique_key<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.unique_key = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for unique_key: {e}"));
            self
        }
        pub fn updated_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.updated_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for updated_at: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSnapshotsItemConfig>
        for super::DbtYmlFilesSnapshotsItemConfig
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSnapshotsItemConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                alias: value.alias?,
                check_cols: value.check_cols?,
                enabled: value.enabled?,
                grants: value.grants?,
                persist_docs: value.persist_docs?,
                post_hook: value.post_hook?,
                pre_hook: value.pre_hook?,
                quote_columns: value.quote_columns?,
                strategy: value.strategy?,
                tags: value.tags?,
                target_database: value.target_database?,
                target_schema: value.target_schema?,
                unique_key: value.unique_key?,
                updated_at: value.updated_at?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSnapshotsItemConfig>
        for DbtYmlFilesSnapshotsItemConfig
    {
        fn from(value: super::DbtYmlFilesSnapshotsItemConfig) -> Self {
            Self {
                alias: Ok(value.alias),
                check_cols: Ok(value.check_cols),
                enabled: Ok(value.enabled),
                grants: Ok(value.grants),
                persist_docs: Ok(value.persist_docs),
                post_hook: Ok(value.post_hook),
                pre_hook: Ok(value.pre_hook),
                quote_columns: Ok(value.quote_columns),
                strategy: Ok(value.strategy),
                tags: Ok(value.tags),
                target_database: Ok(value.target_database),
                target_schema: Ok(value.target_schema),
                unique_key: Ok(value.unique_key),
                updated_at: Ok(value.updated_at),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSourcesItem {
        config: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        data_tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
        database: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        freshness: ::std::result::Result<
            ::std::option::Option<super::FreshnessDefinition>,
            ::std::string::String,
        >,
        loaded_at_field: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        loader: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        overrides: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        quoting: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSourcesItemQuoting>,
            ::std::string::String,
        >,
        schema: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        tables: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesSourcesItemTablesItem>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesSourcesItem {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                database: Ok(Default::default()),
                description: Ok(Default::default()),
                freshness: Ok(Default::default()),
                loaded_at_field: Ok(Default::default()),
                loader: Ok(Default::default()),
                meta: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                overrides: Ok(Default::default()),
                quoting: Ok(Default::default()),
                schema: Ok(Default::default()),
                tables: Ok(Default::default()),
                tags: Ok(Default::default()),
                tests: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSourcesItem {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn freshness<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::FreshnessDefinition>>,
            T::Error: ::std::fmt::Display,
        {
            self.freshness = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for freshness: {e}"));
            self
        }
        pub fn loaded_at_field<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.loaded_at_field = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for loaded_at_field: {e}"));
            self
        }
        pub fn loader<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.loader = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for loader: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn overrides<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.overrides = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for overrides: {e}"));
            self
        }
        pub fn quoting<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtYmlFilesSourcesItemQuoting>>,
            T::Error: ::std::fmt::Display,
        {
            self.quoting = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for quoting: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
        pub fn tables<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesSourcesItemTablesItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.tables = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tables: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tests: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSourcesItem> for super::DbtYmlFilesSourcesItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSourcesItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                data_tests: value.data_tests?,
                database: value.database?,
                description: value.description?,
                freshness: value.freshness?,
                loaded_at_field: value.loaded_at_field?,
                loader: value.loader?,
                meta: value.meta?,
                name: value.name?,
                overrides: value.overrides?,
                quoting: value.quoting?,
                schema: value.schema?,
                tables: value.tables?,
                tags: value.tags?,
                tests: value.tests?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSourcesItem> for DbtYmlFilesSourcesItem {
        fn from(value: super::DbtYmlFilesSourcesItem) -> Self {
            Self {
                config: Ok(value.config),
                data_tests: Ok(value.data_tests),
                database: Ok(value.database),
                description: Ok(value.description),
                freshness: Ok(value.freshness),
                loaded_at_field: Ok(value.loaded_at_field),
                loader: Ok(value.loader),
                meta: Ok(value.meta),
                name: Ok(value.name),
                overrides: Ok(value.overrides),
                quoting: Ok(value.quoting),
                schema: Ok(value.schema),
                tables: Ok(value.tables),
                tags: Ok(value.tags),
                tests: Ok(value.tests),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSourcesItemQuoting {
        database: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        identifier: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        schema: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSourcesItemQuoting {
        fn default() -> Self {
            Self {
                database: Ok(Default::default()),
                identifier: Ok(Default::default()),
                schema: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSourcesItemQuoting {
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
            self
        }
        pub fn identifier<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.identifier = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for identifier: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSourcesItemQuoting>
        for super::DbtYmlFilesSourcesItemQuoting
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSourcesItemQuoting,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                database: value.database?,
                identifier: value.identifier?,
                schema: value.schema?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSourcesItemQuoting> for DbtYmlFilesSourcesItemQuoting {
        fn from(value: super::DbtYmlFilesSourcesItemQuoting) -> Self {
            Self {
                database: Ok(value.database),
                identifier: Ok(value.identifier),
                schema: Ok(value.schema),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSourcesItemTablesItem {
        columns:
            ::std::result::Result<::std::vec::Vec<super::ColumnProperties>, ::std::string::String>,
        config: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSourcesItemTablesItemConfig>,
            ::std::string::String,
        >,
        data_tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        external: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        freshness: ::std::result::Result<
            ::std::option::Option<super::FreshnessDefinition>,
            ::std::string::String,
        >,
        identifier: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        loaded_at_field: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        loader: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        quoting: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesSourcesItemTablesItemQuoting>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        tests: ::std::result::Result<::std::vec::Vec<super::DataTests>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtYmlFilesSourcesItemTablesItem {
        fn default() -> Self {
            Self {
                columns: Ok(Default::default()),
                config: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                description: Ok(Default::default()),
                external: Ok(Default::default()),
                freshness: Ok(Default::default()),
                identifier: Ok(Default::default()),
                loaded_at_field: Ok(Default::default()),
                loader: Ok(Default::default()),
                meta: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                quoting: Ok(Default::default()),
                tags: Ok(Default::default()),
                tests: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSourcesItemTablesItem {
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::ColumnProperties>>,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSourcesItemTablesItemConfig>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn external<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.external = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for external: {e}"));
            self
        }
        pub fn freshness<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::FreshnessDefinition>>,
            T::Error: ::std::fmt::Display,
        {
            self.freshness = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for freshness: {e}"));
            self
        }
        pub fn identifier<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.identifier = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for identifier: {e}"));
            self
        }
        pub fn loaded_at_field<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.loaded_at_field = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for loaded_at_field: {e}"));
            self
        }
        pub fn loader<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.loader = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for loader: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn quoting<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesSourcesItemTablesItemQuoting>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.quoting = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for quoting: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DataTests>>,
            T::Error: ::std::fmt::Display,
        {
            self.tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tests: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSourcesItemTablesItem>
        for super::DbtYmlFilesSourcesItemTablesItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSourcesItemTablesItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                columns: value.columns?,
                config: value.config?,
                data_tests: value.data_tests?,
                description: value.description?,
                external: value.external?,
                freshness: value.freshness?,
                identifier: value.identifier?,
                loaded_at_field: value.loaded_at_field?,
                loader: value.loader?,
                meta: value.meta?,
                name: value.name?,
                quoting: value.quoting?,
                tags: value.tags?,
                tests: value.tests?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSourcesItemTablesItem>
        for DbtYmlFilesSourcesItemTablesItem
    {
        fn from(value: super::DbtYmlFilesSourcesItemTablesItem) -> Self {
            Self {
                columns: Ok(value.columns),
                config: Ok(value.config),
                data_tests: Ok(value.data_tests),
                description: Ok(value.description),
                external: Ok(value.external),
                freshness: Ok(value.freshness),
                identifier: Ok(value.identifier),
                loaded_at_field: Ok(value.loaded_at_field),
                loader: Ok(value.loader),
                meta: Ok(value.meta),
                name: Ok(value.name),
                quoting: Ok(value.quoting),
                tags: Ok(value.tags),
                tests: Ok(value.tests),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSourcesItemTablesItemConfig {
        additional_properties: ::std::result::Result<
            ::std::option::Option<::serde_json::Value>,
            ::std::string::String,
        >,
        event_time: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSourcesItemTablesItemConfig {
        fn default() -> Self {
            Self {
                additional_properties: Ok(Default::default()),
                event_time: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSourcesItemTablesItemConfig {
        pub fn additional_properties<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::serde_json::Value>>,
            T::Error: ::std::fmt::Display,
        {
            self.additional_properties = value.try_into().map_err(|e| {
                format!("error converting supplied value for additional_properties: {e}")
            });
            self
        }
        pub fn event_time<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.event_time = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for event_time: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSourcesItemTablesItemConfig>
        for super::DbtYmlFilesSourcesItemTablesItemConfig
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSourcesItemTablesItemConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                additional_properties: value.additional_properties?,
                event_time: value.event_time?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSourcesItemTablesItemConfig>
        for DbtYmlFilesSourcesItemTablesItemConfig
    {
        fn from(value: super::DbtYmlFilesSourcesItemTablesItemConfig) -> Self {
            Self {
                additional_properties: Ok(value.additional_properties),
                event_time: Ok(value.event_time),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesSourcesItemTablesItemQuoting {
        database: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        identifier: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        schema: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesSourcesItemTablesItemQuoting {
        fn default() -> Self {
            Self {
                database: Ok(Default::default()),
                identifier: Ok(Default::default()),
                schema: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesSourcesItemTablesItemQuoting {
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
            self
        }
        pub fn identifier<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.identifier = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for identifier: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesSourcesItemTablesItemQuoting>
        for super::DbtYmlFilesSourcesItemTablesItemQuoting
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesSourcesItemTablesItemQuoting,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                database: value.database?,
                identifier: value.identifier?,
                schema: value.schema?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesSourcesItemTablesItemQuoting>
        for DbtYmlFilesSourcesItemTablesItemQuoting
    {
        fn from(value: super::DbtYmlFilesSourcesItemTablesItemQuoting) -> Self {
            Self {
                database: Ok(value.database),
                identifier: Ok(value.identifier),
                schema: Ok(value.schema),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesUnitTestsItem {
        config: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesUnitTestsItemConfig>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        expect: ::std::result::Result<super::DbtYmlFilesUnitTestsItemExpect, ::std::string::String>,
        given: ::std::result::Result<
            ::std::vec::Vec<super::DbtYmlFilesUnitTestsItemGivenItem>,
            ::std::string::String,
        >,
        model: ::std::result::Result<::std::string::String, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        overrides: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesUnitTestsItemOverrides>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesUnitTestsItem {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                description: Ok(Default::default()),
                expect: Err("no value supplied for expect".to_string()),
                given: Ok(Default::default()),
                model: Err("no value supplied for model".to_string()),
                name: Err("no value supplied for name".to_string()),
                overrides: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesUnitTestsItem {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesUnitTestsItemConfig>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn expect<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DbtYmlFilesUnitTestsItemExpect>,
            T::Error: ::std::fmt::Display,
        {
            self.expect = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expect: {e}"));
            self
        }
        pub fn given<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtYmlFilesUnitTestsItemGivenItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.given = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for given: {e}"));
            self
        }
        pub fn model<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.model = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for model: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn overrides<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesUnitTestsItemOverrides>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.overrides = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for overrides: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesUnitTestsItem> for super::DbtYmlFilesUnitTestsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesUnitTestsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                description: value.description?,
                expect: value.expect?,
                given: value.given?,
                model: value.model?,
                name: value.name?,
                overrides: value.overrides?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesUnitTestsItem> for DbtYmlFilesUnitTestsItem {
        fn from(value: super::DbtYmlFilesUnitTestsItem) -> Self {
            Self {
                config: Ok(value.config),
                description: Ok(value.description),
                expect: Ok(value.expect),
                given: Ok(value.given),
                model: Ok(value.model),
                name: Ok(value.name),
                overrides: Ok(value.overrides),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesUnitTestsItemConfig {
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesUnitTestsItemConfig {
        fn default() -> Self {
            Self {
                enabled: Ok(Default::default()),
                meta: Ok(Default::default()),
                tags: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesUnitTestsItemConfig {
        pub fn enabled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.enabled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enabled: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesUnitTestsItemConfig>
        for super::DbtYmlFilesUnitTestsItemConfig
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesUnitTestsItemConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                enabled: value.enabled?,
                meta: value.meta?,
                tags: value.tags?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesUnitTestsItemConfig>
        for DbtYmlFilesUnitTestsItemConfig
    {
        fn from(value: super::DbtYmlFilesUnitTestsItemConfig) -> Self {
            Self {
                enabled: Ok(value.enabled),
                meta: Ok(value.meta),
                tags: Ok(value.tags),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesUnitTestsItemExpect {
        fixture: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        format: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesUnitTestsItemExpectFormat>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesUnitTestsItemExpect {
        fn default() -> Self {
            Self {
                fixture: Ok(Default::default()),
                format: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesUnitTestsItemExpect {
        pub fn fixture<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.fixture = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fixture: {e}"));
            self
        }
        pub fn format<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesUnitTestsItemExpectFormat>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.format = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for format: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesUnitTestsItemExpect>
        for super::DbtYmlFilesUnitTestsItemExpect
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesUnitTestsItemExpect,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                fixture: value.fixture?,
                format: value.format?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesUnitTestsItemExpect>
        for DbtYmlFilesUnitTestsItemExpect
    {
        fn from(value: super::DbtYmlFilesUnitTestsItemExpect) -> Self {
            Self {
                fixture: Ok(value.fixture),
                format: Ok(value.format),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesUnitTestsItemGivenItem {
        fixture: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        format: ::std::result::Result<
            ::std::option::Option<super::DbtYmlFilesUnitTestsItemGivenItemFormat>,
            ::std::string::String,
        >,
        input: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesUnitTestsItemGivenItem {
        fn default() -> Self {
            Self {
                fixture: Ok(Default::default()),
                format: Ok(Default::default()),
                input: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesUnitTestsItemGivenItem {
        pub fn fixture<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.fixture = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fixture: {e}"));
            self
        }
        pub fn format<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DbtYmlFilesUnitTestsItemGivenItemFormat>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.format = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for format: {e}"));
            self
        }
        pub fn input<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.input = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for input: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesUnitTestsItemGivenItem>
        for super::DbtYmlFilesUnitTestsItemGivenItem
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesUnitTestsItemGivenItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                fixture: value.fixture?,
                format: value.format?,
                input: value.input?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesUnitTestsItemGivenItem>
        for DbtYmlFilesUnitTestsItemGivenItem
    {
        fn from(value: super::DbtYmlFilesUnitTestsItemGivenItem) -> Self {
            Self {
                fixture: Ok(value.fixture),
                format: Ok(value.format),
                input: Ok(value.input),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtYmlFilesUnitTestsItemOverrides {
        env_vars: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        macros: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        vars: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtYmlFilesUnitTestsItemOverrides {
        fn default() -> Self {
            Self {
                env_vars: Ok(Default::default()),
                macros: Ok(Default::default()),
                vars: Ok(Default::default()),
            }
        }
    }
    impl DbtYmlFilesUnitTestsItemOverrides {
        pub fn env_vars<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.env_vars = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for env_vars: {e}"));
            self
        }
        pub fn macros<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.macros = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for macros: {e}"));
            self
        }
        pub fn vars<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.vars = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for vars: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtYmlFilesUnitTestsItemOverrides>
        for super::DbtYmlFilesUnitTestsItemOverrides
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtYmlFilesUnitTestsItemOverrides,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                env_vars: value.env_vars?,
                macros: value.macros?,
                vars: value.vars?,
            })
        }
    }
    impl ::std::convert::From<super::DbtYmlFilesUnitTestsItemOverrides>
        for DbtYmlFilesUnitTestsItemOverrides
    {
        fn from(value: super::DbtYmlFilesUnitTestsItemOverrides) -> Self {
            Self {
                env_vars: Ok(value.env_vars),
                macros: Ok(value.macros),
                vars: Ok(value.vars),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DerivedMetricTypeParams {
        expr: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        metrics:
            ::std::result::Result<::std::vec::Vec<super::MetricInputSchema>, ::std::string::String>,
    }
    impl ::std::default::Default for DerivedMetricTypeParams {
        fn default() -> Self {
            Self {
                expr: Ok(Default::default()),
                metrics: Ok(Default::default()),
            }
        }
    }
    impl DerivedMetricTypeParams {
        pub fn expr<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.expr = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expr: {e}"));
            self
        }
        pub fn metrics<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::MetricInputSchema>>,
            T::Error: ::std::fmt::Display,
        {
            self.metrics = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metrics: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DerivedMetricTypeParams> for super::DerivedMetricTypeParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DerivedMetricTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                expr: value.expr?,
                metrics: value.metrics?,
            })
        }
    }
    impl ::std::convert::From<super::DerivedMetricTypeParams> for DerivedMetricTypeParams {
        fn from(value: super::DerivedMetricTypeParams) -> Self {
            Self {
                expr: Ok(value.expr),
                metrics: Ok(value.metrics),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Dimension {
        subtype_0: ::std::result::Result<
            ::std::option::Option<super::DimensionSubtype0>,
            ::std::string::String,
        >,
        subtype_1: ::std::result::Result<
            ::std::option::Option<super::DimensionSubtype1>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for Dimension {
        fn default() -> Self {
            Self {
                subtype_0: Ok(Default::default()),
                subtype_1: Ok(Default::default()),
            }
        }
    }
    impl Dimension {
        pub fn subtype_0<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DimensionSubtype0>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_0 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_0: {e}"));
            self
        }
        pub fn subtype_1<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DimensionSubtype1>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_1 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_1: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Dimension> for super::Dimension {
        type Error = super::error::ConversionError;
        fn try_from(
            value: Dimension,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                subtype_0: value.subtype_0?,
                subtype_1: value.subtype_1?,
            })
        }
    }
    impl ::std::convert::From<super::Dimension> for Dimension {
        fn from(value: super::Dimension) -> Self {
            Self {
                subtype_0: Ok(value.subtype_0),
                subtype_1: Ok(value.subtype_1),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DimensionSubtype0 {
        config: ::std::result::Result<
            ::std::option::Option<super::DimensionSubtype0Config>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        expr: ::std::result::Result<
            ::std::option::Option<super::DimensionSubtype0Expr>,
            ::std::string::String,
        >,
        is_partition: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        label: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<super::DimensionSubtype0Name, ::std::string::String>,
        type_: ::std::result::Result<super::DimensionSubtype0Type, ::std::string::String>,
        type_params: ::std::result::Result<
            ::std::option::Option<super::DimensionTypeParams>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DimensionSubtype0 {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                description: Ok(Default::default()),
                expr: Ok(Default::default()),
                is_partition: Ok(Default::default()),
                label: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                type_: Err("no value supplied for type_".to_string()),
                type_params: Ok(Default::default()),
            }
        }
    }
    impl DimensionSubtype0 {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DimensionSubtype0Config>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn expr<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DimensionSubtype0Expr>>,
            T::Error: ::std::fmt::Display,
        {
            self.expr = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expr: {e}"));
            self
        }
        pub fn is_partition<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.is_partition = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for is_partition: {e}"));
            self
        }
        pub fn label<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.label = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for label: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DimensionSubtype0Name>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DimensionSubtype0Type>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
        pub fn type_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DimensionTypeParams>>,
            T::Error: ::std::fmt::Display,
        {
            self.type_params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_params: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DimensionSubtype0> for super::DimensionSubtype0 {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DimensionSubtype0,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                description: value.description?,
                expr: value.expr?,
                is_partition: value.is_partition?,
                label: value.label?,
                name: value.name?,
                type_: value.type_?,
                type_params: value.type_params?,
            })
        }
    }
    impl ::std::convert::From<super::DimensionSubtype0> for DimensionSubtype0 {
        fn from(value: super::DimensionSubtype0) -> Self {
            Self {
                config: Ok(value.config),
                description: Ok(value.description),
                expr: Ok(value.expr),
                is_partition: Ok(value.is_partition),
                label: Ok(value.label),
                name: Ok(value.name),
                type_: Ok(value.type_),
                type_params: Ok(value.type_params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DimensionSubtype0Config {
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DimensionSubtype0Config {
        fn default() -> Self {
            Self {
                meta: Ok(Default::default()),
            }
        }
    }
    impl DimensionSubtype0Config {
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DimensionSubtype0Config> for super::DimensionSubtype0Config {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DimensionSubtype0Config,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { meta: value.meta? })
        }
    }
    impl ::std::convert::From<super::DimensionSubtype0Config> for DimensionSubtype0Config {
        fn from(value: super::DimensionSubtype0Config) -> Self {
            Self {
                meta: Ok(value.meta),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DimensionSubtype1 {
        config: ::std::result::Result<
            ::std::option::Option<super::DimensionSubtype1Config>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        expr: ::std::result::Result<
            ::std::option::Option<super::DimensionSubtype1Expr>,
            ::std::string::String,
        >,
        is_partition: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        label: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<super::DimensionSubtype1Name, ::std::string::String>,
        type_: ::std::result::Result<super::DimensionSubtype1Type, ::std::string::String>,
        type_params: ::std::result::Result<super::DimensionTypeParams, ::std::string::String>,
    }
    impl ::std::default::Default for DimensionSubtype1 {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                description: Ok(Default::default()),
                expr: Ok(Default::default()),
                is_partition: Ok(Default::default()),
                label: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                type_: Err("no value supplied for type_".to_string()),
                type_params: Err("no value supplied for type_params".to_string()),
            }
        }
    }
    impl DimensionSubtype1 {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DimensionSubtype1Config>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn expr<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DimensionSubtype1Expr>>,
            T::Error: ::std::fmt::Display,
        {
            self.expr = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expr: {e}"));
            self
        }
        pub fn is_partition<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.is_partition = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for is_partition: {e}"));
            self
        }
        pub fn label<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.label = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for label: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DimensionSubtype1Name>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DimensionSubtype1Type>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
        pub fn type_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DimensionTypeParams>,
            T::Error: ::std::fmt::Display,
        {
            self.type_params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_params: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DimensionSubtype1> for super::DimensionSubtype1 {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DimensionSubtype1,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                description: value.description?,
                expr: value.expr?,
                is_partition: value.is_partition?,
                label: value.label?,
                name: value.name?,
                type_: value.type_?,
                type_params: value.type_params?,
            })
        }
    }
    impl ::std::convert::From<super::DimensionSubtype1> for DimensionSubtype1 {
        fn from(value: super::DimensionSubtype1) -> Self {
            Self {
                config: Ok(value.config),
                description: Ok(value.description),
                expr: Ok(value.expr),
                is_partition: Ok(value.is_partition),
                label: Ok(value.label),
                name: Ok(value.name),
                type_: Ok(value.type_),
                type_params: Ok(value.type_params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DimensionSubtype1Config {
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DimensionSubtype1Config {
        fn default() -> Self {
            Self {
                meta: Ok(Default::default()),
            }
        }
    }
    impl DimensionSubtype1Config {
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DimensionSubtype1Config> for super::DimensionSubtype1Config {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DimensionSubtype1Config,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { meta: value.meta? })
        }
    }
    impl ::std::convert::From<super::DimensionSubtype1Config> for DimensionSubtype1Config {
        fn from(value: super::DimensionSubtype1Config) -> Self {
            Self {
                meta: Ok(value.meta),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DimensionTypeParams {
        time_granularity:
            ::std::result::Result<super::DimensionTypeParamsTimeGranularity, ::std::string::String>,
        validity_params: ::std::result::Result<
            ::std::option::Option<super::ValidityParams>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DimensionTypeParams {
        fn default() -> Self {
            Self {
                time_granularity: Err("no value supplied for time_granularity".to_string()),
                validity_params: Ok(Default::default()),
            }
        }
    }
    impl DimensionTypeParams {
        pub fn time_granularity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::DimensionTypeParamsTimeGranularity>,
            T::Error: ::std::fmt::Display,
        {
            self.time_granularity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for time_granularity: {e}"));
            self
        }
        pub fn validity_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ValidityParams>>,
            T::Error: ::std::fmt::Display,
        {
            self.validity_params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for validity_params: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DimensionTypeParams> for super::DimensionTypeParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DimensionTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                time_granularity: value.time_granularity?,
                validity_params: value.validity_params?,
            })
        }
    }
    impl ::std::convert::From<super::DimensionTypeParams> for DimensionTypeParams {
        fn from(value: super::DimensionTypeParams) -> Self {
            Self {
                time_granularity: Ok(value.time_granularity),
                validity_params: Ok(value.validity_params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DocsConfig {
        node_color: ::std::result::Result<
            ::std::option::Option<super::DocsConfigNodeColor>,
            ::std::string::String,
        >,
        show: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
    }
    impl ::std::default::Default for DocsConfig {
        fn default() -> Self {
            Self {
                node_color: Ok(Default::default()),
                show: Ok(Default::default()),
            }
        }
    }
    impl DocsConfig {
        pub fn node_color<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DocsConfigNodeColor>>,
            T::Error: ::std::fmt::Display,
        {
            self.node_color = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for node_color: {e}"));
            self
        }
        pub fn show<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.show = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for show: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DocsConfig> for super::DocsConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DocsConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                node_color: value.node_color?,
                show: value.show?,
            })
        }
    }
    impl ::std::convert::From<super::DocsConfig> for DocsConfig {
        fn from(value: super::DocsConfig) -> Self {
            Self {
                node_color: Ok(value.node_color),
                show: Ok(value.show),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Entity {
        config: ::std::result::Result<
            ::std::option::Option<super::EntityConfig>,
            ::std::string::String,
        >,
        entity: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        expr:
            ::std::result::Result<::std::option::Option<super::EntityExpr>, ::std::string::String>,
        name: ::std::result::Result<super::EntityName, ::std::string::String>,
        type_: ::std::result::Result<super::EntityType, ::std::string::String>,
    }
    impl ::std::default::Default for Entity {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                entity: Ok(Default::default()),
                expr: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                type_: Err("no value supplied for type_".to_string()),
            }
        }
    }
    impl Entity {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::EntityConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn entity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.entity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for entity: {e}"));
            self
        }
        pub fn expr<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::EntityExpr>>,
            T::Error: ::std::fmt::Display,
        {
            self.expr = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expr: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::EntityName>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::EntityType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Entity> for super::Entity {
        type Error = super::error::ConversionError;
        fn try_from(value: Entity) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                entity: value.entity?,
                expr: value.expr?,
                name: value.name?,
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::Entity> for Entity {
        fn from(value: super::Entity) -> Self {
            Self {
                config: Ok(value.config),
                entity: Ok(value.entity),
                expr: Ok(value.expr),
                name: Ok(value.name),
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct EntityConfig {
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for EntityConfig {
        fn default() -> Self {
            Self {
                meta: Ok(Default::default()),
            }
        }
    }
    impl EntityConfig {
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<EntityConfig> for super::EntityConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: EntityConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { meta: value.meta? })
        }
    }
    impl ::std::convert::From<super::EntityConfig> for EntityConfig {
        fn from(value: super::EntityConfig) -> Self {
            Self {
                meta: Ok(value.meta),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Export {
        config: ::std::result::Result<
            ::std::option::Option<super::ExportConfig>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for Export {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
            }
        }
    }
    impl Export {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ExportConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Export> for super::Export {
        type Error = super::error::ConversionError;
        fn try_from(value: Export) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                name: value.name?,
            })
        }
    }
    impl ::std::convert::From<super::Export> for Export {
        fn from(value: super::Export) -> Self {
            Self {
                config: Ok(value.config),
                name: Ok(value.name),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ExportConfig {
        alias: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        export_as: ::std::result::Result<
            ::std::option::Option<super::ExportConfigExportAs>,
            ::std::string::String,
        >,
        schema: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ExportConfig {
        fn default() -> Self {
            Self {
                alias: Ok(Default::default()),
                export_as: Ok(Default::default()),
                schema: Ok(Default::default()),
            }
        }
    }
    impl ExportConfig {
        pub fn alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.alias = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for alias: {e}"));
            self
        }
        pub fn export_as<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ExportConfigExportAs>>,
            T::Error: ::std::fmt::Display,
        {
            self.export_as = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for export_as: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ExportConfig> for super::ExportConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ExportConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                alias: value.alias?,
                export_as: value.export_as?,
                schema: value.schema?,
            })
        }
    }
    impl ::std::convert::From<super::ExportConfig> for ExportConfig {
        fn from(value: super::ExportConfig) -> Self {
            Self {
                alias: Ok(value.alias),
                export_as: Ok(value.export_as),
                schema: Ok(value.schema),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct FreshnessDefinition {
        error_after: ::std::result::Result<
            ::std::option::Option<super::FreshnessRules>,
            ::std::string::String,
        >,
        filter: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        warn_after: ::std::result::Result<
            ::std::option::Option<super::FreshnessRules>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for FreshnessDefinition {
        fn default() -> Self {
            Self {
                error_after: Ok(Default::default()),
                filter: Ok(Default::default()),
                warn_after: Ok(Default::default()),
            }
        }
    }
    impl FreshnessDefinition {
        pub fn error_after<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::FreshnessRules>>,
            T::Error: ::std::fmt::Display,
        {
            self.error_after = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error_after: {e}"));
            self
        }
        pub fn filter<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.filter = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for filter: {e}"));
            self
        }
        pub fn warn_after<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::FreshnessRules>>,
            T::Error: ::std::fmt::Display,
        {
            self.warn_after = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for warn_after: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<FreshnessDefinition> for super::FreshnessDefinition {
        type Error = super::error::ConversionError;
        fn try_from(
            value: FreshnessDefinition,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error_after: value.error_after?,
                filter: value.filter?,
                warn_after: value.warn_after?,
            })
        }
    }
    impl ::std::convert::From<super::FreshnessDefinition> for FreshnessDefinition {
        fn from(value: super::FreshnessDefinition) -> Self {
            Self {
                error_after: Ok(value.error_after),
                filter: Ok(value.filter),
                warn_after: Ok(value.warn_after),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct FreshnessRules {
        count: ::std::result::Result<super::NumberOrJinjaString, ::std::string::String>,
        period: ::std::result::Result<super::FreshnessRulesPeriod, ::std::string::String>,
    }
    impl ::std::default::Default for FreshnessRules {
        fn default() -> Self {
            Self {
                count: Err("no value supplied for count".to_string()),
                period: Err("no value supplied for period".to_string()),
            }
        }
    }
    impl FreshnessRules {
        pub fn count<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::NumberOrJinjaString>,
            T::Error: ::std::fmt::Display,
        {
            self.count = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for count: {e}"));
            self
        }
        pub fn period<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::FreshnessRulesPeriod>,
            T::Error: ::std::fmt::Display,
        {
            self.period = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for period: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<FreshnessRules> for super::FreshnessRules {
        type Error = super::error::ConversionError;
        fn try_from(
            value: FreshnessRules,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                count: value.count?,
                period: value.period?,
            })
        }
    }
    impl ::std::convert::From<super::FreshnessRules> for FreshnessRules {
        fn from(value: super::FreshnessRules) -> Self {
            Self {
                count: Ok(value.count),
                period: Ok(value.period),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IncludeExclude {
        exclude: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        include: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for IncludeExclude {
        fn default() -> Self {
            Self {
                exclude: Ok(Default::default()),
                include: Ok(Default::default()),
            }
        }
    }
    impl IncludeExclude {
        pub fn exclude<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.exclude = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for exclude: {e}"));
            self
        }
        pub fn include<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.include = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for include: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IncludeExclude> for super::IncludeExclude {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IncludeExclude,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                exclude: value.exclude?,
                include: value.include?,
            })
        }
    }
    impl ::std::convert::From<super::IncludeExclude> for IncludeExclude {
        fn from(value: super::IncludeExclude) -> Self {
            Self {
                exclude: Ok(value.exclude),
                include: Ok(value.include),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IsTimeDimension {
        type_: ::std::result::Result<super::IsTimeDimensionType, ::std::string::String>,
    }
    impl ::std::default::Default for IsTimeDimension {
        fn default() -> Self {
            Self {
                type_: Err("no value supplied for type_".to_string()),
            }
        }
    }
    impl IsTimeDimension {
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::IsTimeDimensionType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IsTimeDimension> for super::IsTimeDimension {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IsTimeDimension,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::IsTimeDimension> for IsTimeDimension {
        fn from(value: super::IsTimeDimension) -> Self {
            Self {
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Measure {
        agg: ::std::result::Result<super::MeasureAgg, ::std::string::String>,
        agg_params: ::std::result::Result<
            ::std::option::Option<super::AggregationTypeParams>,
            ::std::string::String,
        >,
        agg_time_dimension: ::std::result::Result<
            ::std::option::Option<super::MeasureAggTimeDimension>,
            ::std::string::String,
        >,
        config: ::std::result::Result<
            ::std::option::Option<super::MeasureConfig>,
            ::std::string::String,
        >,
        create_metric: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        create_metric_display_name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        expr:
            ::std::result::Result<::std::option::Option<super::MeasureExpr>, ::std::string::String>,
        label: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<super::MeasureName, ::std::string::String>,
        non_additive_dimension: ::std::result::Result<
            ::std::option::Option<super::NonAdditiveDimension>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for Measure {
        fn default() -> Self {
            Self {
                agg: Err("no value supplied for agg".to_string()),
                agg_params: Ok(Default::default()),
                agg_time_dimension: Ok(Default::default()),
                config: Ok(Default::default()),
                create_metric: Ok(Default::default()),
                create_metric_display_name: Ok(Default::default()),
                description: Ok(Default::default()),
                expr: Ok(Default::default()),
                label: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                non_additive_dimension: Ok(Default::default()),
            }
        }
    }
    impl Measure {
        pub fn agg<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::MeasureAgg>,
            T::Error: ::std::fmt::Display,
        {
            self.agg = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for agg: {e}"));
            self
        }
        pub fn agg_params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::AggregationTypeParams>>,
            T::Error: ::std::fmt::Display,
        {
            self.agg_params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for agg_params: {e}"));
            self
        }
        pub fn agg_time_dimension<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::MeasureAggTimeDimension>>,
            T::Error: ::std::fmt::Display,
        {
            self.agg_time_dimension = value.try_into().map_err(|e| {
                format!("error converting supplied value for agg_time_dimension: {e}")
            });
            self
        }
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::MeasureConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn create_metric<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.create_metric = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for create_metric: {e}"));
            self
        }
        pub fn create_metric_display_name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.create_metric_display_name = value.try_into().map_err(|e| {
                format!("error converting supplied value for create_metric_display_name: {e}")
            });
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {e}"));
            self
        }
        pub fn expr<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::MeasureExpr>>,
            T::Error: ::std::fmt::Display,
        {
            self.expr = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expr: {e}"));
            self
        }
        pub fn label<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.label = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for label: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::MeasureName>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn non_additive_dimension<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::NonAdditiveDimension>>,
            T::Error: ::std::fmt::Display,
        {
            self.non_additive_dimension = value.try_into().map_err(|e| {
                format!("error converting supplied value for non_additive_dimension: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<Measure> for super::Measure {
        type Error = super::error::ConversionError;
        fn try_from(value: Measure) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                agg: value.agg?,
                agg_params: value.agg_params?,
                agg_time_dimension: value.agg_time_dimension?,
                config: value.config?,
                create_metric: value.create_metric?,
                create_metric_display_name: value.create_metric_display_name?,
                description: value.description?,
                expr: value.expr?,
                label: value.label?,
                name: value.name?,
                non_additive_dimension: value.non_additive_dimension?,
            })
        }
    }
    impl ::std::convert::From<super::Measure> for Measure {
        fn from(value: super::Measure) -> Self {
            Self {
                agg: Ok(value.agg),
                agg_params: Ok(value.agg_params),
                agg_time_dimension: Ok(value.agg_time_dimension),
                config: Ok(value.config),
                create_metric: Ok(value.create_metric),
                create_metric_display_name: Ok(value.create_metric_display_name),
                description: Ok(value.description),
                expr: Ok(value.expr),
                label: Ok(value.label),
                name: Ok(value.name),
                non_additive_dimension: Ok(value.non_additive_dimension),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct MeasureConfig {
        meta: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for MeasureConfig {
        fn default() -> Self {
            Self {
                meta: Ok(Default::default()),
            }
        }
    }
    impl MeasureConfig {
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<MeasureConfig> for super::MeasureConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: MeasureConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { meta: value.meta? })
        }
    }
    impl ::std::convert::From<super::MeasureConfig> for MeasureConfig {
        fn from(value: super::MeasureConfig) -> Self {
            Self {
                meta: Ok(value.meta),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ModelConfigs {
        auto_refresh: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        backup: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        contract: ::std::result::Result<
            ::std::option::Option<super::ModelConfigsContract>,
            ::std::string::String,
        >,
        event_time: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        file_format: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        grant_access_to: ::std::result::Result<
            ::std::vec::Vec<super::AuthorizedViewsItem>,
            ::std::string::String,
        >,
        grants: ::std::result::Result<::std::option::Option<super::Grants>, ::std::string::String>,
        hours_to_expiration:
            ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
        kms_key_name: ::std::result::Result<
            ::std::option::Option<super::ModelConfigsKmsKeyName>,
            ::std::string::String,
        >,
        labels: ::std::result::Result<
            ::std::collections::HashMap<
                super::ModelConfigsLabelsKey,
                super::ModelConfigsLabelsValue,
            >,
            ::std::string::String,
        >,
        location: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        materialized: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        on_configuration_change: ::std::result::Result<
            ::std::option::Option<super::ModelConfigsOnConfigurationChange>,
            ::std::string::String,
        >,
        on_schema_change: ::std::result::Result<
            ::std::option::Option<super::ModelConfigsOnSchemaChange>,
            ::std::string::String,
        >,
        snowflake_warehouse: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        sql_header: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        target_lag: ::std::result::Result<
            ::std::option::Option<super::ModelConfigsTargetLag>,
            ::std::string::String,
        >,
        tblproperties: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ModelConfigs {
        fn default() -> Self {
            Self {
                auto_refresh: Ok(Default::default()),
                backup: Ok(Default::default()),
                contract: Ok(Default::default()),
                event_time: Ok(Default::default()),
                file_format: Ok(Default::default()),
                grant_access_to: Ok(Default::default()),
                grants: Ok(Default::default()),
                hours_to_expiration: Ok(Default::default()),
                kms_key_name: Ok(Default::default()),
                labels: Ok(Default::default()),
                location: Ok(Default::default()),
                materialized: Ok(Default::default()),
                on_configuration_change: Ok(Default::default()),
                on_schema_change: Ok(Default::default()),
                snowflake_warehouse: Ok(Default::default()),
                sql_header: Ok(Default::default()),
                target_lag: Ok(Default::default()),
                tblproperties: Ok(Default::default()),
            }
        }
    }
    impl ModelConfigs {
        pub fn auto_refresh<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.auto_refresh = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for auto_refresh: {e}"));
            self
        }
        pub fn backup<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.backup = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for backup: {e}"));
            self
        }
        pub fn contract<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ModelConfigsContract>>,
            T::Error: ::std::fmt::Display,
        {
            self.contract = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for contract: {e}"));
            self
        }
        pub fn event_time<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.event_time = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for event_time: {e}"));
            self
        }
        pub fn file_format<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.file_format = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for file_format: {e}"));
            self
        }
        pub fn grant_access_to<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::AuthorizedViewsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.grant_access_to = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for grant_access_to: {e}"));
            self
        }
        pub fn grants<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Grants>>,
            T::Error: ::std::fmt::Display,
        {
            self.grants = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for grants: {e}"));
            self
        }
        pub fn hours_to_expiration<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<f64>>,
            T::Error: ::std::fmt::Display,
        {
            self.hours_to_expiration = value.try_into().map_err(|e| {
                format!("error converting supplied value for hours_to_expiration: {e}")
            });
            self
        }
        pub fn kms_key_name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ModelConfigsKmsKeyName>>,
            T::Error: ::std::fmt::Display,
        {
            self.kms_key_name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for kms_key_name: {e}"));
            self
        }
        pub fn labels<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<
                    super::ModelConfigsLabelsKey,
                    super::ModelConfigsLabelsValue,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.labels = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for labels: {e}"));
            self
        }
        pub fn location<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.location = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for location: {e}"));
            self
        }
        pub fn materialized<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.materialized = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for materialized: {e}"));
            self
        }
        pub fn on_configuration_change<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::ModelConfigsOnConfigurationChange>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.on_configuration_change = value.try_into().map_err(|e| {
                format!("error converting supplied value for on_configuration_change: {e}")
            });
            self
        }
        pub fn on_schema_change<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ModelConfigsOnSchemaChange>>,
            T::Error: ::std::fmt::Display,
        {
            self.on_schema_change = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for on_schema_change: {e}"));
            self
        }
        pub fn snowflake_warehouse<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.snowflake_warehouse = value.try_into().map_err(|e| {
                format!("error converting supplied value for snowflake_warehouse: {e}")
            });
            self
        }
        pub fn sql_header<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.sql_header = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for sql_header: {e}"));
            self
        }
        pub fn target_lag<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ModelConfigsTargetLag>>,
            T::Error: ::std::fmt::Display,
        {
            self.target_lag = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for target_lag: {e}"));
            self
        }
        pub fn tblproperties<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.tblproperties = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tblproperties: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ModelConfigs> for super::ModelConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ModelConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                auto_refresh: value.auto_refresh?,
                backup: value.backup?,
                contract: value.contract?,
                event_time: value.event_time?,
                file_format: value.file_format?,
                grant_access_to: value.grant_access_to?,
                grants: value.grants?,
                hours_to_expiration: value.hours_to_expiration?,
                kms_key_name: value.kms_key_name?,
                labels: value.labels?,
                location: value.location?,
                materialized: value.materialized?,
                on_configuration_change: value.on_configuration_change?,
                on_schema_change: value.on_schema_change?,
                snowflake_warehouse: value.snowflake_warehouse?,
                sql_header: value.sql_header?,
                target_lag: value.target_lag?,
                tblproperties: value.tblproperties?,
            })
        }
    }
    impl ::std::convert::From<super::ModelConfigs> for ModelConfigs {
        fn from(value: super::ModelConfigs) -> Self {
            Self {
                auto_refresh: Ok(value.auto_refresh),
                backup: Ok(value.backup),
                contract: Ok(value.contract),
                event_time: Ok(value.event_time),
                file_format: Ok(value.file_format),
                grant_access_to: Ok(value.grant_access_to),
                grants: Ok(value.grants),
                hours_to_expiration: Ok(value.hours_to_expiration),
                kms_key_name: Ok(value.kms_key_name),
                labels: Ok(value.labels),
                location: Ok(value.location),
                materialized: Ok(value.materialized),
                on_configuration_change: Ok(value.on_configuration_change),
                on_schema_change: Ok(value.on_schema_change),
                snowflake_warehouse: Ok(value.snowflake_warehouse),
                sql_header: Ok(value.sql_header),
                target_lag: Ok(value.target_lag),
                tblproperties: Ok(value.tblproperties),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ModelConfigsContract {
        enforced: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ModelConfigsContract {
        fn default() -> Self {
            Self {
                enforced: Ok(Default::default()),
            }
        }
    }
    impl ModelConfigsContract {
        pub fn enforced<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.enforced = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enforced: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ModelConfigsContract> for super::ModelConfigsContract {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ModelConfigsContract,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                enforced: value.enforced?,
            })
        }
    }
    impl ::std::convert::From<super::ModelConfigsContract> for ModelConfigsContract {
        fn from(value: super::ModelConfigsContract) -> Self {
            Self {
                enforced: Ok(value.enforced),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct NonAdditiveDimension {
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        window_choice: ::std::result::Result<
            ::std::option::Option<super::NonAdditiveDimensionWindowChoice>,
            ::std::string::String,
        >,
        window_groupings:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
    }
    impl ::std::default::Default for NonAdditiveDimension {
        fn default() -> Self {
            Self {
                name: Err("no value supplied for name".to_string()),
                window_choice: Ok(Default::default()),
                window_groupings: Ok(Default::default()),
            }
        }
    }
    impl NonAdditiveDimension {
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn window_choice<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::NonAdditiveDimensionWindowChoice>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.window_choice = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for window_choice: {e}"));
            self
        }
        pub fn window_groupings<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.window_groupings = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for window_groupings: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<NonAdditiveDimension> for super::NonAdditiveDimension {
        type Error = super::error::ConversionError;
        fn try_from(
            value: NonAdditiveDimension,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                name: value.name?,
                window_choice: value.window_choice?,
                window_groupings: value.window_groupings?,
            })
        }
    }
    impl ::std::convert::From<super::NonAdditiveDimension> for NonAdditiveDimension {
        fn from(value: super::NonAdditiveDimension) -> Self {
            Self {
                name: Ok(value.name),
                window_choice: Ok(value.window_choice),
                window_groupings: Ok(value.window_groupings),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct PersistDocsConfig {
        columns: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        relation: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for PersistDocsConfig {
        fn default() -> Self {
            Self {
                columns: Ok(Default::default()),
                relation: Ok(Default::default()),
            }
        }
    }
    impl PersistDocsConfig {
        pub fn columns<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.columns = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for columns: {e}"));
            self
        }
        pub fn relation<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.relation = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for relation: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<PersistDocsConfig> for super::PersistDocsConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: PersistDocsConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                columns: value.columns?,
                relation: value.relation?,
            })
        }
    }
    impl ::std::convert::From<super::PersistDocsConfig> for PersistDocsConfig {
        fn from(value: super::PersistDocsConfig) -> Self {
            Self {
                columns: Ok(value.columns),
                relation: Ok(value.relation),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RatioMetricTypeParams {
        denominator: ::std::result::Result<
            ::std::option::Option<super::MetricInputSchema>,
            ::std::string::String,
        >,
        filter: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        numerator: ::std::result::Result<
            ::std::option::Option<super::MetricInputSchema>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for RatioMetricTypeParams {
        fn default() -> Self {
            Self {
                denominator: Ok(Default::default()),
                filter: Ok(Default::default()),
                numerator: Ok(Default::default()),
            }
        }
    }
    impl RatioMetricTypeParams {
        pub fn denominator<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::MetricInputSchema>>,
            T::Error: ::std::fmt::Display,
        {
            self.denominator = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for denominator: {e}"));
            self
        }
        pub fn filter<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.filter = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for filter: {e}"));
            self
        }
        pub fn numerator<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::MetricInputSchema>>,
            T::Error: ::std::fmt::Display,
        {
            self.numerator = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for numerator: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RatioMetricTypeParams> for super::RatioMetricTypeParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RatioMetricTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                denominator: value.denominator?,
                filter: value.filter?,
                numerator: value.numerator?,
            })
        }
    }
    impl ::std::convert::From<super::RatioMetricTypeParams> for RatioMetricTypeParams {
        fn from(value: super::RatioMetricTypeParams) -> Self {
            Self {
                denominator: Ok(value.denominator),
                filter: Ok(value.filter),
                numerator: Ok(value.numerator),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RelationshipsTest {
        relationships: ::std::result::Result<
            ::std::option::Option<super::RelationshipsTestRelationships>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for RelationshipsTest {
        fn default() -> Self {
            Self {
                relationships: Ok(Default::default()),
            }
        }
    }
    impl RelationshipsTest {
        pub fn relationships<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::RelationshipsTestRelationships>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.relationships = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for relationships: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RelationshipsTest> for super::RelationshipsTest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RelationshipsTest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                relationships: value.relationships?,
            })
        }
    }
    impl ::std::convert::From<super::RelationshipsTest> for RelationshipsTest {
        fn from(value: super::RelationshipsTest) -> Self {
            Self {
                relationships: Ok(value.relationships),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RelationshipsTestRelationships {
        config:
            ::std::result::Result<::std::option::Option<super::TestConfigs>, ::std::string::String>,
        field: ::std::result::Result<::std::string::String, ::std::string::String>,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        to: ::std::result::Result<::std::string::String, ::std::string::String>,
        where_: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for RelationshipsTestRelationships {
        fn default() -> Self {
            Self {
                config: Ok(Default::default()),
                field: Err("no value supplied for field".to_string()),
                name: Ok(Default::default()),
                to: Err("no value supplied for to".to_string()),
                where_: Ok(Default::default()),
            }
        }
    }
    impl RelationshipsTestRelationships {
        pub fn config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TestConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.config = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config: {e}"));
            self
        }
        pub fn field<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.field = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for field: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn to<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.to = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for to: {e}"));
            self
        }
        pub fn where_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.where_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for where_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RelationshipsTestRelationships>
        for super::RelationshipsTestRelationships
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RelationshipsTestRelationships,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                config: value.config?,
                field: value.field?,
                name: value.name?,
                to: value.to?,
                where_: value.where_?,
            })
        }
    }
    impl ::std::convert::From<super::RelationshipsTestRelationships>
        for RelationshipsTestRelationships
    {
        fn from(value: super::RelationshipsTestRelationships) -> Self {
            Self {
                config: Ok(value.config),
                field: Ok(value.field),
                name: Ok(value.name),
                to: Ok(value.to),
                where_: Ok(value.where_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SimpleMetricTypeParams {
        measure: ::std::result::Result<
            ::std::option::Option<super::MetricInputMeasure>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SimpleMetricTypeParams {
        fn default() -> Self {
            Self {
                measure: Ok(Default::default()),
            }
        }
    }
    impl SimpleMetricTypeParams {
        pub fn measure<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::MetricInputMeasure>>,
            T::Error: ::std::fmt::Display,
        {
            self.measure = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for measure: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SimpleMetricTypeParams> for super::SimpleMetricTypeParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SimpleMetricTypeParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                measure: value.measure?,
            })
        }
    }
    impl ::std::convert::From<super::SimpleMetricTypeParams> for SimpleMetricTypeParams {
        fn from(value: super::SimpleMetricTypeParams) -> Self {
            Self {
                measure: Ok(value.measure),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TestConfigs {
        alias: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        database: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        error_if: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        fail_calc: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        limit: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        schema: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        severity: ::std::result::Result<
            ::std::option::Option<super::TestConfigsSeverity>,
            ::std::string::String,
        >,
        store_failures: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        warn_if: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for TestConfigs {
        fn default() -> Self {
            Self {
                alias: Ok(Default::default()),
                database: Ok(Default::default()),
                enabled: Ok(Default::default()),
                error_if: Ok(Default::default()),
                fail_calc: Ok(Default::default()),
                limit: Ok(Default::default()),
                schema: Ok(Default::default()),
                severity: Ok(Default::default()),
                store_failures: Ok(Default::default()),
                tags: Ok(Default::default()),
                warn_if: Ok(Default::default()),
            }
        }
    }
    impl TestConfigs {
        pub fn alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.alias = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for alias: {e}"));
            self
        }
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
            self
        }
        pub fn enabled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.enabled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enabled: {e}"));
            self
        }
        pub fn error_if<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.error_if = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error_if: {e}"));
            self
        }
        pub fn fail_calc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.fail_calc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fail_calc: {e}"));
            self
        }
        pub fn limit<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.limit = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for limit: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
        pub fn severity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TestConfigsSeverity>>,
            T::Error: ::std::fmt::Display,
        {
            self.severity = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for severity: {e}"));
            self
        }
        pub fn store_failures<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.store_failures = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for store_failures: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn warn_if<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.warn_if = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for warn_if: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<TestConfigs> for super::TestConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TestConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                alias: value.alias?,
                database: value.database?,
                enabled: value.enabled?,
                error_if: value.error_if?,
                fail_calc: value.fail_calc?,
                limit: value.limit?,
                schema: value.schema?,
                severity: value.severity?,
                store_failures: value.store_failures?,
                tags: value.tags?,
                warn_if: value.warn_if?,
            })
        }
    }
    impl ::std::convert::From<super::TestConfigs> for TestConfigs {
        fn from(value: super::TestConfigs) -> Self {
            Self {
                alias: Ok(value.alias),
                database: Ok(value.database),
                enabled: Ok(value.enabled),
                error_if: Ok(value.error_if),
                fail_calc: Ok(value.fail_calc),
                limit: Ok(value.limit),
                schema: Ok(value.schema),
                severity: Ok(value.severity),
                store_failures: Ok(value.store_failures),
                tags: Ok(value.tags),
                warn_if: Ok(value.warn_if),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ValidityParams {
        is_end: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        is_start: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
    }
    impl ::std::default::Default for ValidityParams {
        fn default() -> Self {
            Self {
                is_end: Ok(Default::default()),
                is_start: Ok(Default::default()),
            }
        }
    }
    impl ValidityParams {
        pub fn is_end<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.is_end = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for is_end: {e}"));
            self
        }
        pub fn is_start<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.is_start = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for is_start: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ValidityParams> for super::ValidityParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ValidityParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                is_end: value.is_end?,
                is_start: value.is_start?,
            })
        }
    }
    impl ::std::convert::From<super::ValidityParams> for ValidityParams {
        fn from(value: super::ValidityParams) -> Self {
            Self {
                is_end: Ok(value.is_end),
                is_start: Ok(value.is_start),
            }
        }
    }
}
#[doc = " Error types."]
pub mod error {
    #[doc = r" Error from a `TryFrom` or `FromStr` implementation."]
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
