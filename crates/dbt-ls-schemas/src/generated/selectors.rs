#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

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
#[doc = "`DefinitionBlock`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DefinitionBlock {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub children: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub children_depth: ::std::option::Option<f64>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub childrens_parents: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub indirect_selection: ::std::option::Option<DefinitionBlockIndirectSelection>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub method: ::std::option::Option<DefinitionBlockMethod>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub parents: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub parents_depth: ::std::option::Option<f64>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub value: ::std::option::Option<::std::string::String>,
}
impl DefinitionBlock {
    pub fn builder() -> builder::DefinitionBlock {
        Default::default()
    }
}
#[doc = "`DefinitionBlockIndirectSelection`"]
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
pub enum DefinitionBlockIndirectSelection {
    #[serde(rename = "buildable")]
    Buildable,
    #[serde(rename = "cautious")]
    Cautious,
    #[serde(rename = "eager")]
    Eager,
}
impl ::std::fmt::Display for DefinitionBlockIndirectSelection {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Buildable => f.write_str("buildable"),
            Self::Cautious => f.write_str("cautious"),
            Self::Eager => f.write_str("eager"),
        }
    }
}
impl ::std::str::FromStr for DefinitionBlockIndirectSelection {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "buildable" => Ok(Self::Buildable),
            "cautious" => Ok(Self::Cautious),
            "eager" => Ok(Self::Eager),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DefinitionBlockIndirectSelection {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DefinitionBlockIndirectSelection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DefinitionBlockMethod`"]
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
pub enum DefinitionBlockMethod {
    #[serde(rename = "config")]
    Config,
    #[serde(rename = "exposure")]
    Exposure,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "fqn")]
    Fqn,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "metric")]
    Metric,
    #[serde(rename = "package")]
    Package,
    #[serde(rename = "path")]
    Path,
    #[serde(rename = "result")]
    Result,
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "source_status")]
    SourceStatus,
    #[serde(rename = "state")]
    State,
    #[serde(rename = "tag")]
    Tag,
    #[serde(rename = "test_name")]
    TestName,
    #[serde(rename = "test_type")]
    TestType,
    #[serde(rename = "wildcard")]
    Wildcard,
}
impl ::std::fmt::Display for DefinitionBlockMethod {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Config => f.write_str("config"),
            Self::Exposure => f.write_str("exposure"),
            Self::File => f.write_str("file"),
            Self::Fqn => f.write_str("fqn"),
            Self::Group => f.write_str("group"),
            Self::Metric => f.write_str("metric"),
            Self::Package => f.write_str("package"),
            Self::Path => f.write_str("path"),
            Self::Result => f.write_str("result"),
            Self::Source => f.write_str("source"),
            Self::SourceStatus => f.write_str("source_status"),
            Self::State => f.write_str("state"),
            Self::Tag => f.write_str("tag"),
            Self::TestName => f.write_str("test_name"),
            Self::TestType => f.write_str("test_type"),
            Self::Wildcard => f.write_str("wildcard"),
        }
    }
}
impl ::std::str::FromStr for DefinitionBlockMethod {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "config" => Ok(Self::Config),
            "exposure" => Ok(Self::Exposure),
            "file" => Ok(Self::File),
            "fqn" => Ok(Self::Fqn),
            "group" => Ok(Self::Group),
            "metric" => Ok(Self::Metric),
            "package" => Ok(Self::Package),
            "path" => Ok(Self::Path),
            "result" => Ok(Self::Result),
            "source" => Ok(Self::Source),
            "source_status" => Ok(Self::SourceStatus),
            "state" => Ok(Self::State),
            "tag" => Ok(Self::Tag),
            "test_name" => Ok(Self::TestName),
            "test_type" => Ok(Self::TestType),
            "wildcard" => Ok(Self::Wildcard),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DefinitionBlockMethod {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DefinitionBlockMethod {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ExcludeBlock`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ExcludeBlock(pub ::std::vec::Vec<ExcludeBlockItem>);
impl ::std::ops::Deref for ExcludeBlock {
    type Target = ::std::vec::Vec<ExcludeBlockItem>;
    fn deref(&self) -> &::std::vec::Vec<ExcludeBlockItem> {
        &self.0
    }
}
impl ::std::convert::From<ExcludeBlock> for ::std::vec::Vec<ExcludeBlockItem> {
    fn from(value: ExcludeBlock) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<ExcludeBlockItem>> for ExcludeBlock {
    fn from(value: ::std::vec::Vec<ExcludeBlockItem>) -> Self {
        Self(value)
    }
}
#[doc = "`ExcludeBlockItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ExcludeBlockItem {
    IntersectionBlock(IntersectionBlock),
    DefinitionBlock(DefinitionBlock),
}
impl ::std::convert::From<IntersectionBlock> for ExcludeBlockItem {
    fn from(value: IntersectionBlock) -> Self {
        Self::IntersectionBlock(value)
    }
}
impl ::std::convert::From<DefinitionBlock> for ExcludeBlockItem {
    fn from(value: DefinitionBlock) -> Self {
        Self::DefinitionBlock(value)
    }
}
#[doc = "`IntersectionBlock`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct IntersectionBlock(pub ::std::vec::Vec<DefinitionBlock>);
impl ::std::ops::Deref for IntersectionBlock {
    type Target = ::std::vec::Vec<DefinitionBlock>;
    fn deref(&self) -> &::std::vec::Vec<DefinitionBlock> {
        &self.0
    }
}
impl ::std::convert::From<IntersectionBlock> for ::std::vec::Vec<DefinitionBlock> {
    fn from(value: IntersectionBlock) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<DefinitionBlock>> for IntersectionBlock {
    fn from(value: ::std::vec::Vec<DefinitionBlock>) -> Self {
        Self(value)
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
#[doc = "`Selectors`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Selectors {
    pub selectors: ::std::vec::Vec<SelectorsSelectorsItem>,
}
impl Selectors {
    pub fn builder() -> builder::Selectors {
        Default::default()
    }
}
#[doc = "`SelectorsSelectorsItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct SelectorsSelectorsItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub definition: ::std::option::Option<SelectorsSelectorsItemDefinition>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
}
impl SelectorsSelectorsItem {
    pub fn builder() -> builder::SelectorsSelectorsItem {
        Default::default()
    }
}
#[doc = "`SelectorsSelectorsItemDefinition`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SelectorsSelectorsItemDefinition {
    DefinitionBlock(DefinitionBlock),
    String(::std::string::String),
    UnionBlock(UnionBlock),
}
impl ::std::convert::From<DefinitionBlock> for SelectorsSelectorsItemDefinition {
    fn from(value: DefinitionBlock) -> Self {
        Self::DefinitionBlock(value)
    }
}
impl ::std::convert::From<UnionBlock> for SelectorsSelectorsItemDefinition {
    fn from(value: UnionBlock) -> Self {
        Self::UnionBlock(value)
    }
}
#[doc = "`UnionBlock`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct UnionBlock(pub ::std::vec::Vec<UnionBlockItem>);
impl ::std::ops::Deref for UnionBlock {
    type Target = ::std::vec::Vec<UnionBlockItem>;
    fn deref(&self) -> &::std::vec::Vec<UnionBlockItem> {
        &self.0
    }
}
impl ::std::convert::From<UnionBlock> for ::std::vec::Vec<UnionBlockItem> {
    fn from(value: UnionBlock) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<UnionBlockItem>> for UnionBlock {
    fn from(value: ::std::vec::Vec<UnionBlockItem>) -> Self {
        Self(value)
    }
}
#[doc = "`UnionBlockItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct UnionBlockItem {
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_0: ::std::option::Option<IntersectionBlock>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_1: ::std::option::Option<DefinitionBlock>,
    #[serde(flatten, skip_serializing_if = "::std::option::Option::is_none")]
    pub subtype_2: ::std::option::Option<ExcludeBlock>,
}
impl UnionBlockItem {
    pub fn builder() -> builder::UnionBlockItem {
        Default::default()
    }
}
#[doc = " Types for composing complex structures."]
pub mod builder {
    #[derive(Clone, Debug)]
    pub struct DefinitionBlock {
        children: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        children_depth: ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
        childrens_parents: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        indirect_selection: ::std::result::Result<
            ::std::option::Option<super::DefinitionBlockIndirectSelection>,
            ::std::string::String,
        >,
        method: ::std::result::Result<
            ::std::option::Option<super::DefinitionBlockMethod>,
            ::std::string::String,
        >,
        parents: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        parents_depth: ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
        value: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DefinitionBlock {
        fn default() -> Self {
            Self {
                children: Ok(Default::default()),
                children_depth: Ok(Default::default()),
                childrens_parents: Ok(Default::default()),
                indirect_selection: Ok(Default::default()),
                method: Ok(Default::default()),
                parents: Ok(Default::default()),
                parents_depth: Ok(Default::default()),
                value: Ok(Default::default()),
            }
        }
    }
    impl DefinitionBlock {
        pub fn children<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.children = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for children: {e}"));
            self
        }
        pub fn children_depth<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<f64>>,
            T::Error: ::std::fmt::Display,
        {
            self.children_depth = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for children_depth: {e}"));
            self
        }
        pub fn childrens_parents<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.childrens_parents = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for childrens_parents: {e}"));
            self
        }
        pub fn indirect_selection<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::DefinitionBlockIndirectSelection>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.indirect_selection = value.try_into().map_err(|e| {
                format!("error converting supplied value for indirect_selection: {e}")
            });
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DefinitionBlockMethod>>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {e}"));
            self
        }
        pub fn parents<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.parents = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for parents: {e}"));
            self
        }
        pub fn parents_depth<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<f64>>,
            T::Error: ::std::fmt::Display,
        {
            self.parents_depth = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for parents_depth: {e}"));
            self
        }
        pub fn value<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.value = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for value: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DefinitionBlock> for super::DefinitionBlock {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DefinitionBlock,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                children: value.children?,
                children_depth: value.children_depth?,
                childrens_parents: value.childrens_parents?,
                indirect_selection: value.indirect_selection?,
                method: value.method?,
                parents: value.parents?,
                parents_depth: value.parents_depth?,
                value: value.value?,
            })
        }
    }
    impl ::std::convert::From<super::DefinitionBlock> for DefinitionBlock {
        fn from(value: super::DefinitionBlock) -> Self {
            Self {
                children: Ok(value.children),
                children_depth: Ok(value.children_depth),
                childrens_parents: Ok(value.childrens_parents),
                indirect_selection: Ok(value.indirect_selection),
                method: Ok(value.method),
                parents: Ok(value.parents),
                parents_depth: Ok(value.parents_depth),
                value: Ok(value.value),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Selectors {
        selectors: ::std::result::Result<
            ::std::vec::Vec<super::SelectorsSelectorsItem>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for Selectors {
        fn default() -> Self {
            Self {
                selectors: Err("no value supplied for selectors".to_string()),
            }
        }
    }
    impl Selectors {
        pub fn selectors<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::SelectorsSelectorsItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.selectors = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for selectors: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Selectors> for super::Selectors {
        type Error = super::error::ConversionError;
        fn try_from(
            value: Selectors,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                selectors: value.selectors?,
            })
        }
    }
    impl ::std::convert::From<super::Selectors> for Selectors {
        fn from(value: super::Selectors) -> Self {
            Self {
                selectors: Ok(value.selectors),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SelectorsSelectorsItem {
        definition: ::std::result::Result<
            ::std::option::Option<super::SelectorsSelectorsItemDefinition>,
            ::std::string::String,
        >,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SelectorsSelectorsItem {
        fn default() -> Self {
            Self {
                definition: Ok(Default::default()),
                description: Ok(Default::default()),
                name: Ok(Default::default()),
            }
        }
    }
    impl SelectorsSelectorsItem {
        pub fn definition<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<super::SelectorsSelectorsItemDefinition>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.definition = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for definition: {e}"));
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
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SelectorsSelectorsItem> for super::SelectorsSelectorsItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SelectorsSelectorsItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                definition: value.definition?,
                description: value.description?,
                name: value.name?,
            })
        }
    }
    impl ::std::convert::From<super::SelectorsSelectorsItem> for SelectorsSelectorsItem {
        fn from(value: super::SelectorsSelectorsItem) -> Self {
            Self {
                definition: Ok(value.definition),
                description: Ok(value.description),
                name: Ok(value.name),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct UnionBlockItem {
        subtype_0: ::std::result::Result<
            ::std::option::Option<super::IntersectionBlock>,
            ::std::string::String,
        >,
        subtype_1: ::std::result::Result<
            ::std::option::Option<super::DefinitionBlock>,
            ::std::string::String,
        >,
        subtype_2: ::std::result::Result<
            ::std::option::Option<super::ExcludeBlock>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for UnionBlockItem {
        fn default() -> Self {
            Self {
                subtype_0: Ok(Default::default()),
                subtype_1: Ok(Default::default()),
                subtype_2: Ok(Default::default()),
            }
        }
    }
    impl UnionBlockItem {
        pub fn subtype_0<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::IntersectionBlock>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_0 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_0: {e}"));
            self
        }
        pub fn subtype_1<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DefinitionBlock>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_1 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_1: {e}"));
            self
        }
        pub fn subtype_2<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ExcludeBlock>>,
            T::Error: ::std::fmt::Display,
        {
            self.subtype_2 = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for subtype_2: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<UnionBlockItem> for super::UnionBlockItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: UnionBlockItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                subtype_0: value.subtype_0?,
                subtype_1: value.subtype_1?,
                subtype_2: value.subtype_2?,
            })
        }
    }
    impl ::std::convert::From<super::UnionBlockItem> for UnionBlockItem {
        fn from(value: super::UnionBlockItem) -> Self {
            Self {
                subtype_0: Ok(value.subtype_0),
                subtype_1: Ok(value.subtype_1),
                subtype_2: Ok(value.subtype_2),
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
