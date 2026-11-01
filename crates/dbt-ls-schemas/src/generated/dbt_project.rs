#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

#[doc = "`Access`"]
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
pub enum Access {
    #[serde(rename = "private")]
    Private,
    #[serde(rename = "protected")]
    Protected,
    #[serde(rename = "public")]
    Public,
}
impl ::std::fmt::Display for Access {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Private => f.write_str("private"),
            Self::Protected => f.write_str("protected"),
            Self::Public => f.write_str("public"),
        }
    }
}
impl ::std::str::FromStr for Access {
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
impl ::std::convert::TryFrom<&str> for Access {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Access {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Alias`"]
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
pub struct Alias(pub ::std::string::String);
impl ::std::ops::Deref for Alias {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Alias> for ::std::string::String {
    fn from(value: Alias) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Alias {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Alias {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Alias {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
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
#[doc = "`AutoRefresh`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct AutoRefresh(pub BooleanOrJinjaString);
impl ::std::ops::Deref for AutoRefresh {
    type Target = BooleanOrJinjaString;
    fn deref(&self) -> &BooleanOrJinjaString {
        &self.0
    }
}
impl ::std::convert::From<AutoRefresh> for BooleanOrJinjaString {
    fn from(value: AutoRefresh) -> Self {
        value.0
    }
}
impl ::std::convert::From<BooleanOrJinjaString> for AutoRefresh {
    fn from(value: BooleanOrJinjaString) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for AutoRefresh {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for AutoRefresh {
    type Err = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for AutoRefresh {
    type Error = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for AutoRefresh {
    type Error = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
#[doc = "`Backup`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Backup(pub BooleanOrJinjaString);
impl ::std::ops::Deref for Backup {
    type Target = BooleanOrJinjaString;
    fn deref(&self) -> &BooleanOrJinjaString {
        &self.0
    }
}
impl ::std::convert::From<Backup> for BooleanOrJinjaString {
    fn from(value: Backup) -> Self {
        value.0
    }
}
impl ::std::convert::From<BooleanOrJinjaString> for Backup {
    fn from(value: BooleanOrJinjaString) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Backup {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Backup {
    type Err = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for Backup {
    type Error = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for Backup {
    type Error = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
#[doc = "`BooleanOrJinjaString`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum BooleanOrJinjaString {
    JinjaString(JinjaString),
    Boolean(bool),
}
impl ::std::fmt::Display for BooleanOrJinjaString {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::JinjaString(x) => x.fmt(f),
            Self::Boolean(x) => x.fmt(f),
        }
    }
}
impl ::std::str::FromStr for BooleanOrJinjaString {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if let Ok(v) = value.parse() {
            Ok(Self::JinjaString(v))
        } else if let Ok(v) = value.parse() {
            Ok(Self::Boolean(v))
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
impl ::std::convert::From<JinjaString> for BooleanOrJinjaString {
    fn from(value: JinjaString) -> Self {
        Self::JinjaString(value)
    }
}
impl ::std::convert::From<bool> for BooleanOrJinjaString {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
#[doc = "`ColumnTypes`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ColumnTypes(pub ::std::collections::HashMap<ColumnTypesKey, ::std::string::String>);
impl ::std::ops::Deref for ColumnTypes {
    type Target = ::std::collections::HashMap<ColumnTypesKey, ::std::string::String>;
    fn deref(&self) -> &::std::collections::HashMap<ColumnTypesKey, ::std::string::String> {
        &self.0
    }
}
impl ::std::convert::From<ColumnTypes>
    for ::std::collections::HashMap<ColumnTypesKey, ::std::string::String>
{
    fn from(value: ColumnTypes) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::collections::HashMap<ColumnTypesKey, ::std::string::String>>
    for ColumnTypes
{
    fn from(value: ::std::collections::HashMap<ColumnTypesKey, ::std::string::String>) -> Self {
        Self(value)
    }
}
#[doc = "`ColumnTypesKey`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ColumnTypesKey(::std::string::String);
impl ::std::ops::Deref for ColumnTypesKey {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ColumnTypesKey> for ::std::string::String {
    fn from(value: ColumnTypesKey) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ColumnTypesKey {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for ColumnTypesKey {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ColumnTypesKey {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ColumnTypesKey {
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
#[doc = "`Contract`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Contract {
    pub enforced: BooleanOrJinjaString,
}
impl Contract {
    pub fn builder() -> builder::Contract {
        Default::default()
    }
}
#[doc = "Configurations set in the dbt_project.yml file will apply to all tests that don't have a more specific configuration set."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DataTestConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub alias: ::std::option::Option<Alias>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<Database>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub error_if: ::std::option::Option<ErrorIf>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub fail_calc: ::std::option::Option<FailCalc>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub limit: ::std::option::Option<Limit>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub meta: ::std::option::Option<Meta>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<Schema>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub severity: ::std::option::Option<Severity>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub store_failures: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<Tags>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub warn_if: ::std::option::Option<WarnIf>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, DataTestConfigsExtraValue>,
}
impl DataTestConfigs {
    pub fn builder() -> builder::DataTestConfigs {
        Default::default()
    }
}
#[doc = "`DataTestConfigsExtraValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum DataTestConfigsExtraValue {
    DataTestConfigs(DataTestConfigs),
    EmptyDirectory(EmptyDirectory),
}
impl ::std::convert::From<DataTestConfigs> for DataTestConfigsExtraValue {
    fn from(value: DataTestConfigs) -> Self {
        Self::DataTestConfigs(value)
    }
}
impl ::std::convert::From<EmptyDirectory> for DataTestConfigsExtraValue {
    fn from(value: EmptyDirectory) -> Self {
        Self::EmptyDirectory(value)
    }
}
#[doc = "`Database`"]
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
pub struct Database(pub ::std::string::String);
impl ::std::ops::Deref for Database {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Database> for ::std::string::String {
    fn from(value: Database) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Database {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Database {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Database {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`DbtProject`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtProject {
    #[serde(
        rename = "analysis-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub analysis_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(
        rename = "asset-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub asset_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(
        rename = "clean-targets",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub clean_targets: ::std::option::Option<ArrayOfStrings>,
    #[serde(
        rename = "config-version",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub config_version: ::std::option::Option<i64>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub data_tests: ::std::option::Option<DataTestConfigs>,
    #[serde(
        rename = "dbt-cloud",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub dbt_cloud: ::std::option::Option<DbtProjectDbtCloud>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub dispatch: ::std::vec::Vec<DbtProjectDispatchItem>,
    #[serde(
        rename = "docs-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub docs_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub flags: ::std::option::Option<DbtProjectFlags>,
    #[serde(
        rename = "log-path",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub log_path: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "macro-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub macro_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(
        rename = "model-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub model_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub models: ::std::option::Option<ModelConfigs>,
    pub name: ::std::string::String,
    #[serde(
        rename = "on-run-end",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub on_run_end: ::std::option::Option<DbtProjectOnRunEnd>,
    #[serde(
        rename = "on-run-start",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub on_run_start: ::std::option::Option<DbtProjectOnRunStart>,
    #[serde(
        rename = "packages-install-path",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub packages_install_path: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub profile: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "query-comment",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub query_comment: ::std::option::Option<DbtProjectQueryComment>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quoting: ::std::option::Option<DbtProjectQuoting>,
    #[serde(
        rename = "require-dbt-version",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub require_dbt_version: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(
        rename = "seed-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub seed_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub seeds: ::std::option::Option<SeedConfigs>,
    #[serde(
        rename = "semantic-models",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub semantic_models: ::std::option::Option<SemanticModelConfigs>,
    #[serde(
        rename = "snapshot-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub snapshot_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub snapshots: ::std::option::Option<SnapshotConfigs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub sources: ::std::option::Option<SourceConfigs>,
    #[serde(
        rename = "target-path",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub target_path: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "test-paths",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub test_paths: ::std::option::Option<ArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tests: ::std::option::Option<DataTestConfigs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub unit_tests: ::std::option::Option<UnitTestConfigs>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub vars: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub version: ::std::option::Option<::std::string::String>,
}
impl DbtProject {
    pub fn builder() -> builder::DbtProject {
        Default::default()
    }
}
#[doc = "`DbtProjectDbtCloud`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtProjectDbtCloud {
    #[serde(
        rename = "project-id",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub project_id: ::std::option::Option<f64>,
}
impl DbtProjectDbtCloud {
    pub fn builder() -> builder::DbtProjectDbtCloud {
        Default::default()
    }
}
#[doc = "`DbtProjectDispatchItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DbtProjectDispatchItem {
    pub macro_namespace: ::std::string::String,
    pub search_order: ArrayOfStrings,
}
impl DbtProjectDispatchItem {
    pub fn builder() -> builder::DbtProjectDispatchItem {
        Default::default()
    }
}
#[doc = "`DbtProjectFlags`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtProjectFlags {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub cache_selected_only: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub debug: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub fail_fast: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub indirect_selection: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub log_format: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub log_format_file: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub log_level: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub log_level_file: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub partial_parse: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub populate_cache: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub printer_width: ::std::option::Option<i64>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub require_explicit_package_overrides_for_builtin_materializations:
        ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub require_resource_names_without_spaces: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub send_anonymous_usage_stats: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub source_freshness_run_project_hooks: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub static_parser: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub use_colors: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub use_colors_file: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub use_experimental_parser: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub version_check: ::std::option::Option<bool>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub warn_error: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
    pub warn_error_options: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub write_json: ::std::option::Option<bool>,
}
impl DbtProjectFlags {
    pub fn builder() -> builder::DbtProjectFlags {
        Default::default()
    }
}
#[doc = "`DbtProjectOnRunEnd`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum DbtProjectOnRunEnd {
    ArrayOfStrings(ArrayOfStrings),
    String(::std::string::String),
}
impl ::std::convert::From<ArrayOfStrings> for DbtProjectOnRunEnd {
    fn from(value: ArrayOfStrings) -> Self {
        Self::ArrayOfStrings(value)
    }
}
#[doc = "`DbtProjectOnRunStart`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum DbtProjectOnRunStart {
    ArrayOfStrings(ArrayOfStrings),
    String(::std::string::String),
}
impl ::std::convert::From<ArrayOfStrings> for DbtProjectOnRunStart {
    fn from(value: ArrayOfStrings) -> Self {
        Self::ArrayOfStrings(value)
    }
}
#[doc = "`DbtProjectQueryComment`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged, deny_unknown_fields)]
pub enum DbtProjectQueryComment {
    String(::std::string::String),
    Object {
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        append: ::std::option::Option<BooleanOrJinjaString>,
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        comment: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "job-label",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        job_label: ::std::option::Option<BooleanOrJinjaString>,
    },
}
#[doc = "`DbtProjectQuoting`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
pub struct DbtProjectQuoting {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub identifier: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<BooleanOrJinjaString>,
}
impl DbtProjectQuoting {
    pub fn builder() -> builder::DbtProjectQuoting {
        Default::default()
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
#[doc = "`EmptyDirectory`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct EmptyDirectory(pub ());
impl ::std::ops::Deref for EmptyDirectory {
    type Target = ();
    fn deref(&self) -> &() {
        &self.0
    }
}
impl ::std::convert::From<EmptyDirectory> for () {
    fn from(value: EmptyDirectory) -> Self {
        value.0
    }
}
impl ::std::convert::From<()> for EmptyDirectory {
    fn from(value: ()) -> Self {
        Self(value)
    }
}
#[doc = "`ErrorIf`"]
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
pub struct ErrorIf(pub ::std::string::String);
impl ::std::ops::Deref for ErrorIf {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ErrorIf> for ::std::string::String {
    fn from(value: ErrorIf) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for ErrorIf {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for ErrorIf {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for ErrorIf {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`FailCalc`"]
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
pub struct FailCalc(pub ::std::string::String);
impl ::std::ops::Deref for FailCalc {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<FailCalc> for ::std::string::String {
    fn from(value: FailCalc) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for FailCalc {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for FailCalc {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for FailCalc {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`FileFormat`"]
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
pub struct FileFormat(pub ::std::string::String);
impl ::std::ops::Deref for FileFormat {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<FileFormat> for ::std::string::String {
    fn from(value: FileFormat) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for FileFormat {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for FileFormat {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "Configuration, specific to BigQuery adapter, used to setup authorized views."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct GrantAccessTo(pub ::std::vec::Vec<GrantAccessToItem>);
impl ::std::ops::Deref for GrantAccessTo {
    type Target = ::std::vec::Vec<GrantAccessToItem>;
    fn deref(&self) -> &::std::vec::Vec<GrantAccessToItem> {
        &self.0
    }
}
impl ::std::convert::From<GrantAccessTo> for ::std::vec::Vec<GrantAccessToItem> {
    fn from(value: GrantAccessTo) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::vec::Vec<GrantAccessToItem>> for GrantAccessTo {
    fn from(value: ::std::vec::Vec<GrantAccessToItem>) -> Self {
        Self(value)
    }
}
#[doc = "`GrantAccessToItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct GrantAccessToItem {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub dataset: ::std::option::Option<::std::string::String>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub project: ::std::option::Option<::std::string::String>,
}
impl GrantAccessToItem {
    pub fn builder() -> builder::GrantAccessToItem {
        Default::default()
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
#[doc = "Configuration specific to BigQuery adapter used to set an expiration delay (in hours) to a table."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct HoursToExpiration(pub f64);
impl ::std::ops::Deref for HoursToExpiration {
    type Target = f64;
    fn deref(&self) -> &f64 {
        &self.0
    }
}
impl ::std::convert::From<HoursToExpiration> for f64 {
    fn from(value: HoursToExpiration) -> Self {
        value.0
    }
}
impl ::std::convert::From<f64> for HoursToExpiration {
    fn from(value: f64) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for HoursToExpiration {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for HoursToExpiration {
    type Err = <f64 as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for HoursToExpiration {
    type Error = <f64 as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for HoursToExpiration {
    type Error = <f64 as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
#[doc = "`IncrementalStrategy`"]
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
pub struct IncrementalStrategy(pub ::std::string::String);
impl ::std::ops::Deref for IncrementalStrategy {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<IncrementalStrategy> for ::std::string::String {
    fn from(value: IncrementalStrategy) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for IncrementalStrategy {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for IncrementalStrategy {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for IncrementalStrategy {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`InvalidateHardDeletes`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct InvalidateHardDeletes(pub BooleanOrJinjaString);
impl ::std::ops::Deref for InvalidateHardDeletes {
    type Target = BooleanOrJinjaString;
    fn deref(&self) -> &BooleanOrJinjaString {
        &self.0
    }
}
impl ::std::convert::From<InvalidateHardDeletes> for BooleanOrJinjaString {
    fn from(value: InvalidateHardDeletes) -> Self {
        value.0
    }
}
impl ::std::convert::From<BooleanOrJinjaString> for InvalidateHardDeletes {
    fn from(value: BooleanOrJinjaString) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for InvalidateHardDeletes {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for InvalidateHardDeletes {
    type Err = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for InvalidateHardDeletes {
    type Error = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for InvalidateHardDeletes {
    type Error = <BooleanOrJinjaString as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
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
#[doc = "Configuration, specific to BigQuery adapter, of the KMS key name used for data encryption."]
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
pub struct KmsKeyName(pub ::std::string::String);
impl ::std::ops::Deref for KmsKeyName {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<KmsKeyName> for ::std::string::String {
    fn from(value: KmsKeyName) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for KmsKeyName {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for KmsKeyName {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for KmsKeyName {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "Configurations specific to BigQuery adapter used to add labels and tags to tables & views created by dbt."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct LabelConfigs(pub ::std::collections::HashMap<LabelConfigsKey, LabelConfigsValue>);
impl ::std::ops::Deref for LabelConfigs {
    type Target = ::std::collections::HashMap<LabelConfigsKey, LabelConfigsValue>;
    fn deref(&self) -> &::std::collections::HashMap<LabelConfigsKey, LabelConfigsValue> {
        &self.0
    }
}
impl ::std::convert::From<LabelConfigs>
    for ::std::collections::HashMap<LabelConfigsKey, LabelConfigsValue>
{
    fn from(value: LabelConfigs) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::collections::HashMap<LabelConfigsKey, LabelConfigsValue>>
    for LabelConfigs
{
    fn from(value: ::std::collections::HashMap<LabelConfigsKey, LabelConfigsValue>) -> Self {
        Self(value)
    }
}
#[doc = "`LabelConfigsKey`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct LabelConfigsKey(::std::string::String);
impl ::std::ops::Deref for LabelConfigsKey {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<LabelConfigsKey> for ::std::string::String {
    fn from(value: LabelConfigsKey) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for LabelConfigsKey {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("^[a-z][a-z0-9_-]{0,63}$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-z][a-z0-9_-]{0,63}$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for LabelConfigsKey {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for LabelConfigsKey {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for LabelConfigsKey {
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
#[doc = "`LabelConfigsValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum LabelConfigsValue {
    String(LabelConfigsValueString),
    JinjaString(JinjaString),
}
impl ::std::fmt::Display for LabelConfigsValue {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::String(x) => x.fmt(f),
            Self::JinjaString(x) => x.fmt(f),
        }
    }
}
impl ::std::str::FromStr for LabelConfigsValue {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if let Ok(v) = value.parse() {
            Ok(Self::String(v))
        } else if let Ok(v) = value.parse() {
            Ok(Self::JinjaString(v))
        } else {
            Err("string conversion failed for all variants".into())
        }
    }
}
impl ::std::convert::TryFrom<&str> for LabelConfigsValue {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for LabelConfigsValue {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::From<LabelConfigsValueString> for LabelConfigsValue {
    fn from(value: LabelConfigsValueString) -> Self {
        Self::String(value)
    }
}
impl ::std::convert::From<JinjaString> for LabelConfigsValue {
    fn from(value: JinjaString) -> Self {
        Self::JinjaString(value)
    }
}
#[doc = "`LabelConfigsValueString`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct LabelConfigsValueString(::std::string::String);
impl ::std::ops::Deref for LabelConfigsValueString {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<LabelConfigsValueString> for ::std::string::String {
    fn from(value: LabelConfigsValueString) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for LabelConfigsValueString {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[a-z0-9_-]{0,64}$").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-z0-9_-]{0,64}$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for LabelConfigsValueString {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for LabelConfigsValueString {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for LabelConfigsValueString {
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
#[doc = "`Limit`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Limit(pub i64);
impl ::std::ops::Deref for Limit {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<Limit> for i64 {
    fn from(value: Limit) -> Self {
        value.0
    }
}
impl ::std::convert::From<i64> for Limit {
    fn from(value: i64) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Limit {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Limit {
    type Err = <i64 as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for Limit {
    type Error = <i64 as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for Limit {
    type Error = <i64 as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
#[doc = "`Location`"]
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
pub struct Location(pub ::std::string::String);
impl ::std::ops::Deref for Location {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Location> for ::std::string::String {
    fn from(value: Location) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Location {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Location {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Location {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`Materialized`"]
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
pub struct Materialized(pub ::std::string::String);
impl ::std::ops::Deref for Materialized {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Materialized> for ::std::string::String {
    fn from(value: Materialized) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Materialized {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Materialized {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Materialized {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`Meta`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Meta(pub ::serde_json::Map<::std::string::String, ::serde_json::Value>);
impl ::std::ops::Deref for Meta {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<Meta> for ::serde_json::Map<::std::string::String, ::serde_json::Value> {
    fn from(value: Meta) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>> for Meta {
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
#[doc = "Configurations set in the dbt_project.yml file will apply to all models that don't have a more specific configuration set."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ModelConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub access: ::std::option::Option<Access>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub alias: ::std::option::Option<Alias>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub auto_refresh: ::std::option::Option<AutoRefresh>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub backup: ::std::option::Option<Backup>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub bind: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub contract: ::std::option::Option<Contract>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub copy_grants: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<Database>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub file_format: ::std::option::Option<FileFormat>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub full_refresh: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grant_access_to: ::std::option::Option<GrantAccessTo>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grants: ::std::option::Option<Grants>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub hours_to_expiration: ::std::option::Option<HoursToExpiration>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub incremental_strategy: ::std::option::Option<IncrementalStrategy>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub kms_key_name: ::std::option::Option<KmsKeyName>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub labels: ::std::option::Option<LabelConfigs>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub location: ::std::option::Option<Location>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub materialized: ::std::option::Option<Materialized>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub meta: ::std::option::Option<Meta>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub on_configuration_change: ::std::option::Option<OnConfigurationChange>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub on_schema_change: ::std::option::Option<OnSchemaChange>,
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
    pub schema: ::std::option::Option<Schema>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub secure: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub snowflake_warehouse: ::std::option::Option<SnowflakeWarehouse>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub sql_header: ::std::option::Option<SqlHeader>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<Tags>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub target_lag: ::std::option::Option<TargetLag>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tblproperties: ::std::option::Option<Tblproperties>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub transient: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, ModelConfigsExtraValue>,
}
impl ModelConfigs {
    pub fn builder() -> builder::ModelConfigs {
        Default::default()
    }
}
#[doc = "`ModelConfigsExtraValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ModelConfigsExtraValue {
    ModelConfigs(ModelConfigs),
    EmptyDirectory(EmptyDirectory),
}
impl ::std::convert::From<ModelConfigs> for ModelConfigsExtraValue {
    fn from(value: ModelConfigs) -> Self {
        Self::ModelConfigs(value)
    }
}
impl ::std::convert::From<EmptyDirectory> for ModelConfigsExtraValue {
    fn from(value: EmptyDirectory) -> Self {
        Self::EmptyDirectory(value)
    }
}
#[doc = "`OnConfigurationChange`"]
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
pub enum OnConfigurationChange {
    #[serde(rename = "apply")]
    Apply,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "fail")]
    Fail,
}
impl ::std::fmt::Display for OnConfigurationChange {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Apply => f.write_str("apply"),
            Self::Continue => f.write_str("continue"),
            Self::Fail => f.write_str("fail"),
        }
    }
}
impl ::std::str::FromStr for OnConfigurationChange {
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
impl ::std::convert::TryFrom<&str> for OnConfigurationChange {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OnConfigurationChange {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`OnSchemaChange`"]
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
pub enum OnSchemaChange {
    #[serde(rename = "append_new_columns")]
    AppendNewColumns,
    #[serde(rename = "fail")]
    Fail,
    #[serde(rename = "ignore")]
    Ignore,
    #[serde(rename = "sync_all_columns")]
    SyncAllColumns,
}
impl ::std::fmt::Display for OnSchemaChange {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::AppendNewColumns => f.write_str("append_new_columns"),
            Self::Fail => f.write_str("fail"),
            Self::Ignore => f.write_str("ignore"),
            Self::SyncAllColumns => f.write_str("sync_all_columns"),
        }
    }
}
impl ::std::str::FromStr for OnSchemaChange {
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
impl ::std::convert::TryFrom<&str> for OnSchemaChange {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OnSchemaChange {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
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
#[doc = "`Schema`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Schema(pub ::std::option::Option<::std::string::String>);
impl ::std::ops::Deref for Schema {
    type Target = ::std::option::Option<::std::string::String>;
    fn deref(&self) -> &::std::option::Option<::std::string::String> {
        &self.0
    }
}
impl ::std::convert::From<Schema> for ::std::option::Option<::std::string::String> {
    fn from(value: Schema) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::option::Option<::std::string::String>> for Schema {
    fn from(value: ::std::option::Option<::std::string::String>) -> Self {
        Self(value)
    }
}
#[doc = "`SeedConfigs`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SeedConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub column_types: ::std::option::Option<ColumnTypes>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub copy_grants: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<Database>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub full_refresh: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grants: ::std::option::Option<Grants>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub meta: ::std::option::Option<Meta>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub persist_docs: ::std::option::Option<PersistDocsConfig>,
    #[serde(
        rename = "post-hook",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub post_hook: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(
        rename = "pre-hook",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pre_hook: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub quote_columns: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub schema: ::std::option::Option<Schema>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub snowflake_warehouse: ::std::option::Option<SnowflakeWarehouse>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<Tags>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub transient: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, SeedConfigsExtraValue>,
}
impl SeedConfigs {
    pub fn builder() -> builder::SeedConfigs {
        Default::default()
    }
}
#[doc = "`SeedConfigsExtraValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SeedConfigsExtraValue {
    SeedConfigs(SeedConfigs),
    EmptyDirectory(EmptyDirectory),
}
impl ::std::convert::From<SeedConfigs> for SeedConfigsExtraValue {
    fn from(value: SeedConfigs) -> Self {
        Self::SeedConfigs(value)
    }
}
impl ::std::convert::From<EmptyDirectory> for SeedConfigsExtraValue {
    fn from(value: EmptyDirectory) -> Self {
        Self::EmptyDirectory(value)
    }
}
#[doc = "Configurations set in the dbt_project.yml file will apply to all semantic models that don't have a more specific configuration set."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SemanticModelConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub meta: ::std::option::Option<Meta>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, SemanticModelConfigsExtraValue>,
}
impl SemanticModelConfigs {
    pub fn builder() -> builder::SemanticModelConfigs {
        Default::default()
    }
}
#[doc = "`SemanticModelConfigsExtraValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SemanticModelConfigsExtraValue {
    SemanticModelConfigs(SemanticModelConfigs),
    EmptyDirectory(EmptyDirectory),
}
impl ::std::convert::From<SemanticModelConfigs> for SemanticModelConfigsExtraValue {
    fn from(value: SemanticModelConfigs) -> Self {
        Self::SemanticModelConfigs(value)
    }
}
impl ::std::convert::From<EmptyDirectory> for SemanticModelConfigsExtraValue {
    fn from(value: EmptyDirectory) -> Self {
        Self::EmptyDirectory(value)
    }
}
#[doc = "`Severity`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Severity {
    JinjaString(JinjaString),
    String(SeverityString),
}
impl ::std::convert::From<JinjaString> for Severity {
    fn from(value: JinjaString) -> Self {
        Self::JinjaString(value)
    }
}
impl ::std::convert::From<SeverityString> for Severity {
    fn from(value: SeverityString) -> Self {
        Self::String(value)
    }
}
#[doc = "`SeverityString`"]
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
pub enum SeverityString {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warn")]
    Warn,
}
impl ::std::fmt::Display for SeverityString {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Error => f.write_str("error"),
            Self::Warn => f.write_str("warn"),
        }
    }
}
impl ::std::str::FromStr for SeverityString {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SeverityString {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SeverityString {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SnapshotConfigs`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SnapshotConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub alias: ::std::option::Option<Alias>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub check_cols: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub database: ::std::option::Option<Database>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub docs: ::std::option::Option<DocsConfig>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub grants: ::std::option::Option<Grants>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub group: ::std::option::Option<Group>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub invalidate_hard_deletes: ::std::option::Option<InvalidateHardDeletes>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub meta: ::std::option::Option<Meta>,
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
    pub strategy: ::std::option::Option<Strategy>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<Tags>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub target_database: ::std::option::Option<TargetDatabase>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub target_schema: ::std::option::Option<TargetSchema>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub transient: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub unique_key: ::std::option::Option<StringOrArrayOfStrings>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub updated_at: ::std::option::Option<UpdatedAt>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, SnapshotConfigsExtraValue>,
}
impl SnapshotConfigs {
    pub fn builder() -> builder::SnapshotConfigs {
        Default::default()
    }
}
#[doc = "`SnapshotConfigsExtraValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SnapshotConfigsExtraValue {
    SnapshotConfigs(SnapshotConfigs),
    EmptyDirectory(EmptyDirectory),
}
impl ::std::convert::From<SnapshotConfigs> for SnapshotConfigsExtraValue {
    fn from(value: SnapshotConfigs) -> Self {
        Self::SnapshotConfigs(value)
    }
}
impl ::std::convert::From<EmptyDirectory> for SnapshotConfigsExtraValue {
    fn from(value: EmptyDirectory) -> Self {
        Self::EmptyDirectory(value)
    }
}
#[doc = "`SnowflakeWarehouse`"]
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
pub struct SnowflakeWarehouse(pub ::std::string::String);
impl ::std::ops::Deref for SnowflakeWarehouse {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SnowflakeWarehouse> for ::std::string::String {
    fn from(value: SnowflakeWarehouse) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for SnowflakeWarehouse {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for SnowflakeWarehouse {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for SnowflakeWarehouse {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`SourceConfigs`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SourceConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub meta: ::std::option::Option<Meta>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<Tags>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, SourceConfigsExtraValue>,
}
impl SourceConfigs {
    pub fn builder() -> builder::SourceConfigs {
        Default::default()
    }
}
#[doc = "`SourceConfigsExtraValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SourceConfigsExtraValue {
    SourceConfigs(SourceConfigs),
    EmptyDirectory(EmptyDirectory),
}
impl ::std::convert::From<SourceConfigs> for SourceConfigsExtraValue {
    fn from(value: SourceConfigs) -> Self {
        Self::SourceConfigs(value)
    }
}
impl ::std::convert::From<EmptyDirectory> for SourceConfigsExtraValue {
    fn from(value: EmptyDirectory) -> Self {
        Self::EmptyDirectory(value)
    }
}
#[doc = "`SqlHeader`"]
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
pub struct SqlHeader(pub ::std::string::String);
impl ::std::ops::Deref for SqlHeader {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<SqlHeader> for ::std::string::String {
    fn from(value: SqlHeader) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for SqlHeader {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for SqlHeader {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for SqlHeader {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`Strategy`"]
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
pub struct Strategy(pub ::std::string::String);
impl ::std::ops::Deref for Strategy {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Strategy> for ::std::string::String {
    fn from(value: Strategy) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Strategy {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Strategy {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
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
#[doc = "`Tags`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Tags(pub StringOrArrayOfStrings);
impl ::std::ops::Deref for Tags {
    type Target = StringOrArrayOfStrings;
    fn deref(&self) -> &StringOrArrayOfStrings {
        &self.0
    }
}
impl ::std::convert::From<Tags> for StringOrArrayOfStrings {
    fn from(value: Tags) -> Self {
        value.0
    }
}
impl ::std::convert::From<StringOrArrayOfStrings> for Tags {
    fn from(value: StringOrArrayOfStrings) -> Self {
        Self(value)
    }
}
#[doc = "`TargetDatabase`"]
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
pub struct TargetDatabase(pub ::std::string::String);
impl ::std::ops::Deref for TargetDatabase {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TargetDatabase> for ::std::string::String {
    fn from(value: TargetDatabase) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for TargetDatabase {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for TargetDatabase {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for TargetDatabase {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`TargetLag`"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct TargetLag(::std::string::String);
impl ::std::ops::Deref for TargetLag {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TargetLag> for ::std::string::String {
    fn from(value: TargetLag) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for TargetLag {
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
impl ::std::convert::TryFrom<&str> for TargetLag {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TargetLag {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for TargetLag {
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
#[doc = "`TargetSchema`"]
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
pub struct TargetSchema(pub ::std::string::String);
impl ::std::ops::Deref for TargetSchema {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<TargetSchema> for ::std::string::String {
    fn from(value: TargetSchema) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for TargetSchema {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for TargetSchema {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for TargetSchema {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`Tblproperties`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Tblproperties(pub ::serde_json::Map<::std::string::String, ::serde_json::Value>);
impl ::std::ops::Deref for Tblproperties {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<Tblproperties>
    for ::serde_json::Map<::std::string::String, ::serde_json::Value>
{
    fn from(value: Tblproperties) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
    for Tblproperties
{
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
#[doc = "`UnitTestConfigs`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UnitTestConfigs {
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub enabled: ::std::option::Option<BooleanOrJinjaString>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub meta: ::std::option::Option<Meta>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<Tags>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, UnitTestConfigsExtraValue>,
}
impl UnitTestConfigs {
    pub fn builder() -> builder::UnitTestConfigs {
        Default::default()
    }
}
#[doc = "`UnitTestConfigsExtraValue`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum UnitTestConfigsExtraValue {
    UnitTestConfigs(UnitTestConfigs),
    EmptyDirectory(EmptyDirectory),
}
impl ::std::convert::From<UnitTestConfigs> for UnitTestConfigsExtraValue {
    fn from(value: UnitTestConfigs) -> Self {
        Self::UnitTestConfigs(value)
    }
}
impl ::std::convert::From<EmptyDirectory> for UnitTestConfigsExtraValue {
    fn from(value: EmptyDirectory) -> Self {
        Self::EmptyDirectory(value)
    }
}
#[doc = "`UpdatedAt`"]
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
pub struct UpdatedAt(pub ::std::string::String);
impl ::std::ops::Deref for UpdatedAt {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<UpdatedAt> for ::std::string::String {
    fn from(value: UpdatedAt) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for UpdatedAt {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for UpdatedAt {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for UpdatedAt {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`WarnIf`"]
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
pub struct WarnIf(pub ::std::string::String);
impl ::std::ops::Deref for WarnIf {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<WarnIf> for ::std::string::String {
    fn from(value: WarnIf) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for WarnIf {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for WarnIf {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for WarnIf {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = " Types for composing complex structures."]
pub mod builder {
    #[derive(Clone, Debug)]
    pub struct Contract {
        enforced: ::std::result::Result<super::BooleanOrJinjaString, ::std::string::String>,
    }
    impl ::std::default::Default for Contract {
        fn default() -> Self {
            Self {
                enforced: Err("no value supplied for enforced".to_string()),
            }
        }
    }
    impl Contract {
        pub fn enforced<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::BooleanOrJinjaString>,
            T::Error: ::std::fmt::Display,
        {
            self.enforced = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for enforced: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Contract> for super::Contract {
        type Error = super::error::ConversionError;
        fn try_from(value: Contract) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                enforced: value.enforced?,
            })
        }
    }
    impl ::std::convert::From<super::Contract> for Contract {
        fn from(value: super::Contract) -> Self {
            Self {
                enforced: Ok(value.enforced),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DataTestConfigs {
        alias: ::std::result::Result<::std::option::Option<super::Alias>, ::std::string::String>,
        database:
            ::std::result::Result<::std::option::Option<super::Database>, ::std::string::String>,
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        error_if:
            ::std::result::Result<::std::option::Option<super::ErrorIf>, ::std::string::String>,
        fail_calc:
            ::std::result::Result<::std::option::Option<super::FailCalc>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        limit: ::std::result::Result<::std::option::Option<super::Limit>, ::std::string::String>,
        meta: ::std::result::Result<::std::option::Option<super::Meta>, ::std::string::String>,
        schema: ::std::result::Result<::std::option::Option<super::Schema>, ::std::string::String>,
        severity:
            ::std::result::Result<::std::option::Option<super::Severity>, ::std::string::String>,
        store_failures: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<::std::option::Option<super::Tags>, ::std::string::String>,
        warn_if: ::std::result::Result<::std::option::Option<super::WarnIf>, ::std::string::String>,
        extra: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::DataTestConfigsExtraValue>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DataTestConfigs {
        fn default() -> Self {
            Self {
                alias: Ok(Default::default()),
                database: Ok(Default::default()),
                enabled: Ok(Default::default()),
                error_if: Ok(Default::default()),
                fail_calc: Ok(Default::default()),
                group: Ok(Default::default()),
                limit: Ok(Default::default()),
                meta: Ok(Default::default()),
                schema: Ok(Default::default()),
                severity: Ok(Default::default()),
                store_failures: Ok(Default::default()),
                tags: Ok(Default::default()),
                warn_if: Ok(Default::default()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl DataTestConfigs {
        pub fn alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Alias>>,
            T::Error: ::std::fmt::Display,
        {
            self.alias = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for alias: {e}"));
            self
        }
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Database>>,
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
            T: ::std::convert::TryInto<::std::option::Option<super::ErrorIf>>,
            T::Error: ::std::fmt::Display,
        {
            self.error_if = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error_if: {e}"));
            self
        }
        pub fn fail_calc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::FailCalc>>,
            T::Error: ::std::fmt::Display,
        {
            self.fail_calc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fail_calc: {e}"));
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
        pub fn limit<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Limit>>,
            T::Error: ::std::fmt::Display,
        {
            self.limit = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for limit: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Meta>>,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Schema>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
        pub fn severity<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Severity>>,
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
            T: ::std::convert::TryInto<::std::option::Option<super::Tags>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn warn_if<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::WarnIf>>,
            T::Error: ::std::fmt::Display,
        {
            self.warn_if = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for warn_if: {e}"));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<
                    ::std::string::String,
                    super::DataTestConfigsExtraValue,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DataTestConfigs> for super::DataTestConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DataTestConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                alias: value.alias?,
                database: value.database?,
                enabled: value.enabled?,
                error_if: value.error_if?,
                fail_calc: value.fail_calc?,
                group: value.group?,
                limit: value.limit?,
                meta: value.meta?,
                schema: value.schema?,
                severity: value.severity?,
                store_failures: value.store_failures?,
                tags: value.tags?,
                warn_if: value.warn_if?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::DataTestConfigs> for DataTestConfigs {
        fn from(value: super::DataTestConfigs) -> Self {
            Self {
                alias: Ok(value.alias),
                database: Ok(value.database),
                enabled: Ok(value.enabled),
                error_if: Ok(value.error_if),
                fail_calc: Ok(value.fail_calc),
                group: Ok(value.group),
                limit: Ok(value.limit),
                meta: Ok(value.meta),
                schema: Ok(value.schema),
                severity: Ok(value.severity),
                store_failures: Ok(value.store_failures),
                tags: Ok(value.tags),
                warn_if: Ok(value.warn_if),
                extra: Ok(value.extra),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtProject {
        analysis_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        asset_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        clean_targets: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        config_version: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        data_tests: ::std::result::Result<
            ::std::option::Option<super::DataTestConfigs>,
            ::std::string::String,
        >,
        dbt_cloud: ::std::result::Result<
            ::std::option::Option<super::DbtProjectDbtCloud>,
            ::std::string::String,
        >,
        dispatch: ::std::result::Result<
            ::std::vec::Vec<super::DbtProjectDispatchItem>,
            ::std::string::String,
        >,
        docs_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        flags: ::std::result::Result<
            ::std::option::Option<super::DbtProjectFlags>,
            ::std::string::String,
        >,
        log_path: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        macro_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        model_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        models: ::std::result::Result<
            ::std::option::Option<super::ModelConfigs>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        on_run_end: ::std::result::Result<
            ::std::option::Option<super::DbtProjectOnRunEnd>,
            ::std::string::String,
        >,
        on_run_start: ::std::result::Result<
            ::std::option::Option<super::DbtProjectOnRunStart>,
            ::std::string::String,
        >,
        packages_install_path: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        profile: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        query_comment: ::std::result::Result<
            ::std::option::Option<super::DbtProjectQueryComment>,
            ::std::string::String,
        >,
        quoting: ::std::result::Result<
            ::std::option::Option<super::DbtProjectQuoting>,
            ::std::string::String,
        >,
        require_dbt_version: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        seed_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        seeds:
            ::std::result::Result<::std::option::Option<super::SeedConfigs>, ::std::string::String>,
        semantic_models: ::std::result::Result<
            ::std::option::Option<super::SemanticModelConfigs>,
            ::std::string::String,
        >,
        snapshot_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        snapshots: ::std::result::Result<
            ::std::option::Option<super::SnapshotConfigs>,
            ::std::string::String,
        >,
        sources: ::std::result::Result<
            ::std::option::Option<super::SourceConfigs>,
            ::std::string::String,
        >,
        target_path: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        test_paths: ::std::result::Result<
            ::std::option::Option<super::ArrayOfStrings>,
            ::std::string::String,
        >,
        tests: ::std::result::Result<
            ::std::option::Option<super::DataTestConfigs>,
            ::std::string::String,
        >,
        unit_tests: ::std::result::Result<
            ::std::option::Option<super::UnitTestConfigs>,
            ::std::string::String,
        >,
        vars: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        version: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DbtProject {
        fn default() -> Self {
            Self {
                analysis_paths: Ok(Default::default()),
                asset_paths: Ok(Default::default()),
                clean_targets: Ok(Default::default()),
                config_version: Ok(Default::default()),
                data_tests: Ok(Default::default()),
                dbt_cloud: Ok(Default::default()),
                dispatch: Ok(Default::default()),
                docs_paths: Ok(Default::default()),
                flags: Ok(Default::default()),
                log_path: Ok(Default::default()),
                macro_paths: Ok(Default::default()),
                model_paths: Ok(Default::default()),
                models: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                on_run_end: Ok(Default::default()),
                on_run_start: Ok(Default::default()),
                packages_install_path: Ok(Default::default()),
                profile: Ok(Default::default()),
                query_comment: Ok(Default::default()),
                quoting: Ok(Default::default()),
                require_dbt_version: Ok(Default::default()),
                seed_paths: Ok(Default::default()),
                seeds: Ok(Default::default()),
                semantic_models: Ok(Default::default()),
                snapshot_paths: Ok(Default::default()),
                snapshots: Ok(Default::default()),
                sources: Ok(Default::default()),
                target_path: Ok(Default::default()),
                test_paths: Ok(Default::default()),
                tests: Ok(Default::default()),
                unit_tests: Ok(Default::default()),
                vars: Ok(Default::default()),
                version: Ok(Default::default()),
            }
        }
    }
    impl DbtProject {
        pub fn analysis_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.analysis_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for analysis_paths: {e}"));
            self
        }
        pub fn asset_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_paths: {e}"));
            self
        }
        pub fn clean_targets<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.clean_targets = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for clean_targets: {e}"));
            self
        }
        pub fn config_version<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.config_version = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for config_version: {e}"));
            self
        }
        pub fn data_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DataTestConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.data_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data_tests: {e}"));
            self
        }
        pub fn dbt_cloud<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtProjectDbtCloud>>,
            T::Error: ::std::fmt::Display,
        {
            self.dbt_cloud = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for dbt_cloud: {e}"));
            self
        }
        pub fn dispatch<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::DbtProjectDispatchItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.dispatch = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for dispatch: {e}"));
            self
        }
        pub fn docs_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.docs_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for docs_paths: {e}"));
            self
        }
        pub fn flags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtProjectFlags>>,
            T::Error: ::std::fmt::Display,
        {
            self.flags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for flags: {e}"));
            self
        }
        pub fn log_path<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.log_path = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for log_path: {e}"));
            self
        }
        pub fn macro_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.macro_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for macro_paths: {e}"));
            self
        }
        pub fn model_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.model_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for model_paths: {e}"));
            self
        }
        pub fn models<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ModelConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.models = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for models: {e}"));
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
        pub fn on_run_end<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtProjectOnRunEnd>>,
            T::Error: ::std::fmt::Display,
        {
            self.on_run_end = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for on_run_end: {e}"));
            self
        }
        pub fn on_run_start<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtProjectOnRunStart>>,
            T::Error: ::std::fmt::Display,
        {
            self.on_run_start = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for on_run_start: {e}"));
            self
        }
        pub fn packages_install_path<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.packages_install_path = value.try_into().map_err(|e| {
                format!("error converting supplied value for packages_install_path: {e}")
            });
            self
        }
        pub fn profile<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.profile = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for profile: {e}"));
            self
        }
        pub fn query_comment<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtProjectQueryComment>>,
            T::Error: ::std::fmt::Display,
        {
            self.query_comment = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for query_comment: {e}"));
            self
        }
        pub fn quoting<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DbtProjectQuoting>>,
            T::Error: ::std::fmt::Display,
        {
            self.quoting = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for quoting: {e}"));
            self
        }
        pub fn require_dbt_version<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.require_dbt_version = value.try_into().map_err(|e| {
                format!("error converting supplied value for require_dbt_version: {e}")
            });
            self
        }
        pub fn seed_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.seed_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for seed_paths: {e}"));
            self
        }
        pub fn seeds<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::SeedConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.seeds = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for seeds: {e}"));
            self
        }
        pub fn semantic_models<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::SemanticModelConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.semantic_models = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for semantic_models: {e}"));
            self
        }
        pub fn snapshot_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.snapshot_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for snapshot_paths: {e}"));
            self
        }
        pub fn snapshots<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::SnapshotConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.snapshots = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for snapshots: {e}"));
            self
        }
        pub fn sources<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::SourceConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.sources = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for sources: {e}"));
            self
        }
        pub fn target_path<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.target_path = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for target_path: {e}"));
            self
        }
        pub fn test_paths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.test_paths = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for test_paths: {e}"));
            self
        }
        pub fn tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::DataTestConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tests: {e}"));
            self
        }
        pub fn unit_tests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::UnitTestConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.unit_tests = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for unit_tests: {e}"));
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
        pub fn version<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.version = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for version: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtProject> for super::DbtProject {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtProject,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                analysis_paths: value.analysis_paths?,
                asset_paths: value.asset_paths?,
                clean_targets: value.clean_targets?,
                config_version: value.config_version?,
                data_tests: value.data_tests?,
                dbt_cloud: value.dbt_cloud?,
                dispatch: value.dispatch?,
                docs_paths: value.docs_paths?,
                flags: value.flags?,
                log_path: value.log_path?,
                macro_paths: value.macro_paths?,
                model_paths: value.model_paths?,
                models: value.models?,
                name: value.name?,
                on_run_end: value.on_run_end?,
                on_run_start: value.on_run_start?,
                packages_install_path: value.packages_install_path?,
                profile: value.profile?,
                query_comment: value.query_comment?,
                quoting: value.quoting?,
                require_dbt_version: value.require_dbt_version?,
                seed_paths: value.seed_paths?,
                seeds: value.seeds?,
                semantic_models: value.semantic_models?,
                snapshot_paths: value.snapshot_paths?,
                snapshots: value.snapshots?,
                sources: value.sources?,
                target_path: value.target_path?,
                test_paths: value.test_paths?,
                tests: value.tests?,
                unit_tests: value.unit_tests?,
                vars: value.vars?,
                version: value.version?,
            })
        }
    }
    impl ::std::convert::From<super::DbtProject> for DbtProject {
        fn from(value: super::DbtProject) -> Self {
            Self {
                analysis_paths: Ok(value.analysis_paths),
                asset_paths: Ok(value.asset_paths),
                clean_targets: Ok(value.clean_targets),
                config_version: Ok(value.config_version),
                data_tests: Ok(value.data_tests),
                dbt_cloud: Ok(value.dbt_cloud),
                dispatch: Ok(value.dispatch),
                docs_paths: Ok(value.docs_paths),
                flags: Ok(value.flags),
                log_path: Ok(value.log_path),
                macro_paths: Ok(value.macro_paths),
                model_paths: Ok(value.model_paths),
                models: Ok(value.models),
                name: Ok(value.name),
                on_run_end: Ok(value.on_run_end),
                on_run_start: Ok(value.on_run_start),
                packages_install_path: Ok(value.packages_install_path),
                profile: Ok(value.profile),
                query_comment: Ok(value.query_comment),
                quoting: Ok(value.quoting),
                require_dbt_version: Ok(value.require_dbt_version),
                seed_paths: Ok(value.seed_paths),
                seeds: Ok(value.seeds),
                semantic_models: Ok(value.semantic_models),
                snapshot_paths: Ok(value.snapshot_paths),
                snapshots: Ok(value.snapshots),
                sources: Ok(value.sources),
                target_path: Ok(value.target_path),
                test_paths: Ok(value.test_paths),
                tests: Ok(value.tests),
                unit_tests: Ok(value.unit_tests),
                vars: Ok(value.vars),
                version: Ok(value.version),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtProjectDbtCloud {
        project_id: ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtProjectDbtCloud {
        fn default() -> Self {
            Self {
                project_id: Ok(Default::default()),
            }
        }
    }
    impl DbtProjectDbtCloud {
        pub fn project_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<f64>>,
            T::Error: ::std::fmt::Display,
        {
            self.project_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for project_id: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtProjectDbtCloud> for super::DbtProjectDbtCloud {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtProjectDbtCloud,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                project_id: value.project_id?,
            })
        }
    }
    impl ::std::convert::From<super::DbtProjectDbtCloud> for DbtProjectDbtCloud {
        fn from(value: super::DbtProjectDbtCloud) -> Self {
            Self {
                project_id: Ok(value.project_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtProjectDispatchItem {
        macro_namespace: ::std::result::Result<::std::string::String, ::std::string::String>,
        search_order: ::std::result::Result<super::ArrayOfStrings, ::std::string::String>,
    }
    impl ::std::default::Default for DbtProjectDispatchItem {
        fn default() -> Self {
            Self {
                macro_namespace: Err("no value supplied for macro_namespace".to_string()),
                search_order: Err("no value supplied for search_order".to_string()),
            }
        }
    }
    impl DbtProjectDispatchItem {
        pub fn macro_namespace<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.macro_namespace = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for macro_namespace: {e}"));
            self
        }
        pub fn search_order<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::ArrayOfStrings>,
            T::Error: ::std::fmt::Display,
        {
            self.search_order = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for search_order: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtProjectDispatchItem> for super::DbtProjectDispatchItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtProjectDispatchItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                macro_namespace: value.macro_namespace?,
                search_order: value.search_order?,
            })
        }
    }
    impl ::std::convert::From<super::DbtProjectDispatchItem> for DbtProjectDispatchItem {
        fn from(value: super::DbtProjectDispatchItem) -> Self {
            Self {
                macro_namespace: Ok(value.macro_namespace),
                search_order: Ok(value.search_order),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtProjectFlags {
        cache_selected_only:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        debug: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        fail_fast: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        indirect_selection: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        log_format: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        log_format_file: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        log_level: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        log_level_file: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        partial_parse: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        populate_cache: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        printer_width: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        require_explicit_package_overrides_for_builtin_materializations:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        require_resource_names_without_spaces:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        send_anonymous_usage_stats:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        source_freshness_run_project_hooks:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        static_parser: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        use_colors: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        use_colors_file: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        use_experimental_parser:
            ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        version_check: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        warn_error: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        warn_error_options: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        write_json: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
    }
    impl ::std::default::Default for DbtProjectFlags {
        fn default() -> Self {
            Self {
                cache_selected_only: Ok(Default::default()),
                debug: Ok(Default::default()),
                fail_fast: Ok(Default::default()),
                indirect_selection: Ok(Default::default()),
                log_format: Ok(Default::default()),
                log_format_file: Ok(Default::default()),
                log_level: Ok(Default::default()),
                log_level_file: Ok(Default::default()),
                partial_parse: Ok(Default::default()),
                populate_cache: Ok(Default::default()),
                printer_width: Ok(Default::default()),
                require_explicit_package_overrides_for_builtin_materializations: Ok(
                    Default::default(),
                ),
                require_resource_names_without_spaces: Ok(Default::default()),
                send_anonymous_usage_stats: Ok(Default::default()),
                source_freshness_run_project_hooks: Ok(Default::default()),
                static_parser: Ok(Default::default()),
                use_colors: Ok(Default::default()),
                use_colors_file: Ok(Default::default()),
                use_experimental_parser: Ok(Default::default()),
                version_check: Ok(Default::default()),
                warn_error: Ok(Default::default()),
                warn_error_options: Ok(Default::default()),
                write_json: Ok(Default::default()),
            }
        }
    }
    impl DbtProjectFlags {
        pub fn cache_selected_only<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.cache_selected_only = value.try_into().map_err(|e| {
                format!("error converting supplied value for cache_selected_only: {e}")
            });
            self
        }
        pub fn debug<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.debug = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for debug: {e}"));
            self
        }
        pub fn fail_fast<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.fail_fast = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fail_fast: {e}"));
            self
        }
        pub fn indirect_selection<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.indirect_selection = value.try_into().map_err(|e| {
                format!("error converting supplied value for indirect_selection: {e}")
            });
            self
        }
        pub fn log_format<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.log_format = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for log_format: {e}"));
            self
        }
        pub fn log_format_file<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.log_format_file = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for log_format_file: {e}"));
            self
        }
        pub fn log_level<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.log_level = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for log_level: {e}"));
            self
        }
        pub fn log_level_file<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.log_level_file = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for log_level_file: {e}"));
            self
        }
        pub fn partial_parse<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.partial_parse = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for partial_parse: {e}"));
            self
        }
        pub fn populate_cache<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.populate_cache = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for populate_cache: {e}"));
            self
        }
        pub fn printer_width<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.printer_width = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for printer_width: {e}"));
            self
        }
        pub fn require_explicit_package_overrides_for_builtin_materializations<T>(
            mut self,
            value: T,
        ) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self . require_explicit_package_overrides_for_builtin_materializations = value . try_into () . map_err (| e | format ! ("error converting supplied value for require_explicit_package_overrides_for_builtin_materializations: {e}")) ;
            self
        }
        pub fn require_resource_names_without_spaces<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self . require_resource_names_without_spaces = value . try_into () . map_err (| e | format ! ("error converting supplied value for require_resource_names_without_spaces: {e}")) ;
            self
        }
        pub fn send_anonymous_usage_stats<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.send_anonymous_usage_stats = value.try_into().map_err(|e| {
                format!("error converting supplied value for send_anonymous_usage_stats: {e}")
            });
            self
        }
        pub fn source_freshness_run_project_hooks<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.source_freshness_run_project_hooks = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for source_freshness_run_project_hooks: {e}"
                )
            });
            self
        }
        pub fn static_parser<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.static_parser = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for static_parser: {e}"));
            self
        }
        pub fn use_colors<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.use_colors = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for use_colors: {e}"));
            self
        }
        pub fn use_colors_file<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.use_colors_file = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for use_colors_file: {e}"));
            self
        }
        pub fn use_experimental_parser<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.use_experimental_parser = value.try_into().map_err(|e| {
                format!("error converting supplied value for use_experimental_parser: {e}")
            });
            self
        }
        pub fn version_check<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.version_check = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for version_check: {e}"));
            self
        }
        pub fn warn_error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.warn_error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for warn_error: {e}"));
            self
        }
        pub fn warn_error_options<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.warn_error_options = value.try_into().map_err(|e| {
                format!("error converting supplied value for warn_error_options: {e}")
            });
            self
        }
        pub fn write_json<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.write_json = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for write_json: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DbtProjectFlags> for super::DbtProjectFlags {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtProjectFlags,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                cache_selected_only: value.cache_selected_only?,
                debug: value.debug?,
                fail_fast: value.fail_fast?,
                indirect_selection: value.indirect_selection?,
                log_format: value.log_format?,
                log_format_file: value.log_format_file?,
                log_level: value.log_level?,
                log_level_file: value.log_level_file?,
                partial_parse: value.partial_parse?,
                populate_cache: value.populate_cache?,
                printer_width: value.printer_width?,
                require_explicit_package_overrides_for_builtin_materializations: value
                    .require_explicit_package_overrides_for_builtin_materializations?,
                require_resource_names_without_spaces: value
                    .require_resource_names_without_spaces?,
                send_anonymous_usage_stats: value.send_anonymous_usage_stats?,
                source_freshness_run_project_hooks: value.source_freshness_run_project_hooks?,
                static_parser: value.static_parser?,
                use_colors: value.use_colors?,
                use_colors_file: value.use_colors_file?,
                use_experimental_parser: value.use_experimental_parser?,
                version_check: value.version_check?,
                warn_error: value.warn_error?,
                warn_error_options: value.warn_error_options?,
                write_json: value.write_json?,
            })
        }
    }
    impl ::std::convert::From<super::DbtProjectFlags> for DbtProjectFlags {
        fn from(value: super::DbtProjectFlags) -> Self {
            Self {
                cache_selected_only: Ok(value.cache_selected_only),
                debug: Ok(value.debug),
                fail_fast: Ok(value.fail_fast),
                indirect_selection: Ok(value.indirect_selection),
                log_format: Ok(value.log_format),
                log_format_file: Ok(value.log_format_file),
                log_level: Ok(value.log_level),
                log_level_file: Ok(value.log_level_file),
                partial_parse: Ok(value.partial_parse),
                populate_cache: Ok(value.populate_cache),
                printer_width: Ok(value.printer_width),
                require_explicit_package_overrides_for_builtin_materializations: Ok(
                    value.require_explicit_package_overrides_for_builtin_materializations
                ),
                require_resource_names_without_spaces: Ok(
                    value.require_resource_names_without_spaces
                ),
                send_anonymous_usage_stats: Ok(value.send_anonymous_usage_stats),
                source_freshness_run_project_hooks: Ok(value.source_freshness_run_project_hooks),
                static_parser: Ok(value.static_parser),
                use_colors: Ok(value.use_colors),
                use_colors_file: Ok(value.use_colors_file),
                use_experimental_parser: Ok(value.use_experimental_parser),
                version_check: Ok(value.version_check),
                warn_error: Ok(value.warn_error),
                warn_error_options: Ok(value.warn_error_options),
                write_json: Ok(value.write_json),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DbtProjectQuoting {
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
    impl ::std::default::Default for DbtProjectQuoting {
        fn default() -> Self {
            Self {
                database: Ok(Default::default()),
                identifier: Ok(Default::default()),
                schema: Ok(Default::default()),
            }
        }
    }
    impl DbtProjectQuoting {
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
    impl ::std::convert::TryFrom<DbtProjectQuoting> for super::DbtProjectQuoting {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DbtProjectQuoting,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                database: value.database?,
                identifier: value.identifier?,
                schema: value.schema?,
            })
        }
    }
    impl ::std::convert::From<super::DbtProjectQuoting> for DbtProjectQuoting {
        fn from(value: super::DbtProjectQuoting) -> Self {
            Self {
                database: Ok(value.database),
                identifier: Ok(value.identifier),
                schema: Ok(value.schema),
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
    pub struct GrantAccessToItem {
        dataset: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        project: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for GrantAccessToItem {
        fn default() -> Self {
            Self {
                dataset: Ok(Default::default()),
                project: Ok(Default::default()),
            }
        }
    }
    impl GrantAccessToItem {
        pub fn dataset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.dataset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for dataset: {e}"));
            self
        }
        pub fn project<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.project = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for project: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GrantAccessToItem> for super::GrantAccessToItem {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GrantAccessToItem,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                dataset: value.dataset?,
                project: value.project?,
            })
        }
    }
    impl ::std::convert::From<super::GrantAccessToItem> for GrantAccessToItem {
        fn from(value: super::GrantAccessToItem) -> Self {
            Self {
                dataset: Ok(value.dataset),
                project: Ok(value.project),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ModelConfigs {
        access: ::std::result::Result<::std::option::Option<super::Access>, ::std::string::String>,
        alias: ::std::result::Result<::std::option::Option<super::Alias>, ::std::string::String>,
        auto_refresh:
            ::std::result::Result<::std::option::Option<super::AutoRefresh>, ::std::string::String>,
        backup: ::std::result::Result<::std::option::Option<super::Backup>, ::std::string::String>,
        bind: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        contract:
            ::std::result::Result<::std::option::Option<super::Contract>, ::std::string::String>,
        copy_grants: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        database:
            ::std::result::Result<::std::option::Option<super::Database>, ::std::string::String>,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        file_format:
            ::std::result::Result<::std::option::Option<super::FileFormat>, ::std::string::String>,
        full_refresh: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        grant_access_to: ::std::result::Result<
            ::std::option::Option<super::GrantAccessTo>,
            ::std::string::String,
        >,
        grants: ::std::result::Result<::std::option::Option<super::Grants>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        hours_to_expiration: ::std::result::Result<
            ::std::option::Option<super::HoursToExpiration>,
            ::std::string::String,
        >,
        incremental_strategy: ::std::result::Result<
            ::std::option::Option<super::IncrementalStrategy>,
            ::std::string::String,
        >,
        kms_key_name:
            ::std::result::Result<::std::option::Option<super::KmsKeyName>, ::std::string::String>,
        labels: ::std::result::Result<
            ::std::option::Option<super::LabelConfigs>,
            ::std::string::String,
        >,
        location:
            ::std::result::Result<::std::option::Option<super::Location>, ::std::string::String>,
        materialized: ::std::result::Result<
            ::std::option::Option<super::Materialized>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<::std::option::Option<super::Meta>, ::std::string::String>,
        on_configuration_change: ::std::result::Result<
            ::std::option::Option<super::OnConfigurationChange>,
            ::std::string::String,
        >,
        on_schema_change: ::std::result::Result<
            ::std::option::Option<super::OnSchemaChange>,
            ::std::string::String,
        >,
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
        schema: ::std::result::Result<::std::option::Option<super::Schema>, ::std::string::String>,
        secure: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        snowflake_warehouse: ::std::result::Result<
            ::std::option::Option<super::SnowflakeWarehouse>,
            ::std::string::String,
        >,
        sql_header:
            ::std::result::Result<::std::option::Option<super::SqlHeader>, ::std::string::String>,
        tags: ::std::result::Result<::std::option::Option<super::Tags>, ::std::string::String>,
        target_lag:
            ::std::result::Result<::std::option::Option<super::TargetLag>, ::std::string::String>,
        tblproperties: ::std::result::Result<
            ::std::option::Option<super::Tblproperties>,
            ::std::string::String,
        >,
        transient: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        extra: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::ModelConfigsExtraValue>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ModelConfigs {
        fn default() -> Self {
            Self {
                access: Ok(Default::default()),
                alias: Ok(Default::default()),
                auto_refresh: Ok(Default::default()),
                backup: Ok(Default::default()),
                bind: Ok(Default::default()),
                contract: Ok(Default::default()),
                copy_grants: Ok(Default::default()),
                database: Ok(Default::default()),
                docs: Ok(Default::default()),
                enabled: Ok(Default::default()),
                file_format: Ok(Default::default()),
                full_refresh: Ok(Default::default()),
                grant_access_to: Ok(Default::default()),
                grants: Ok(Default::default()),
                group: Ok(Default::default()),
                hours_to_expiration: Ok(Default::default()),
                incremental_strategy: Ok(Default::default()),
                kms_key_name: Ok(Default::default()),
                labels: Ok(Default::default()),
                location: Ok(Default::default()),
                materialized: Ok(Default::default()),
                meta: Ok(Default::default()),
                on_configuration_change: Ok(Default::default()),
                on_schema_change: Ok(Default::default()),
                persist_docs: Ok(Default::default()),
                post_hook: Ok(Default::default()),
                pre_hook: Ok(Default::default()),
                schema: Ok(Default::default()),
                secure: Ok(Default::default()),
                snowflake_warehouse: Ok(Default::default()),
                sql_header: Ok(Default::default()),
                tags: Ok(Default::default()),
                target_lag: Ok(Default::default()),
                tblproperties: Ok(Default::default()),
                transient: Ok(Default::default()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl ModelConfigs {
        pub fn access<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Access>>,
            T::Error: ::std::fmt::Display,
        {
            self.access = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for access: {e}"));
            self
        }
        pub fn alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Alias>>,
            T::Error: ::std::fmt::Display,
        {
            self.alias = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for alias: {e}"));
            self
        }
        pub fn auto_refresh<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::AutoRefresh>>,
            T::Error: ::std::fmt::Display,
        {
            self.auto_refresh = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for auto_refresh: {e}"));
            self
        }
        pub fn backup<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Backup>>,
            T::Error: ::std::fmt::Display,
        {
            self.backup = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for backup: {e}"));
            self
        }
        pub fn bind<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.bind = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for bind: {e}"));
            self
        }
        pub fn contract<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Contract>>,
            T::Error: ::std::fmt::Display,
        {
            self.contract = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for contract: {e}"));
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
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Database>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
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
        pub fn file_format<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::FileFormat>>,
            T::Error: ::std::fmt::Display,
        {
            self.file_format = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for file_format: {e}"));
            self
        }
        pub fn full_refresh<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.full_refresh = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for full_refresh: {e}"));
            self
        }
        pub fn grant_access_to<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::GrantAccessTo>>,
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
        pub fn hours_to_expiration<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::HoursToExpiration>>,
            T::Error: ::std::fmt::Display,
        {
            self.hours_to_expiration = value.try_into().map_err(|e| {
                format!("error converting supplied value for hours_to_expiration: {e}")
            });
            self
        }
        pub fn incremental_strategy<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::IncrementalStrategy>>,
            T::Error: ::std::fmt::Display,
        {
            self.incremental_strategy = value.try_into().map_err(|e| {
                format!("error converting supplied value for incremental_strategy: {e}")
            });
            self
        }
        pub fn kms_key_name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::KmsKeyName>>,
            T::Error: ::std::fmt::Display,
        {
            self.kms_key_name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for kms_key_name: {e}"));
            self
        }
        pub fn labels<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::LabelConfigs>>,
            T::Error: ::std::fmt::Display,
        {
            self.labels = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for labels: {e}"));
            self
        }
        pub fn location<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Location>>,
            T::Error: ::std::fmt::Display,
        {
            self.location = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for location: {e}"));
            self
        }
        pub fn materialized<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Materialized>>,
            T::Error: ::std::fmt::Display,
        {
            self.materialized = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for materialized: {e}"));
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Meta>>,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn on_configuration_change<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::OnConfigurationChange>>,
            T::Error: ::std::fmt::Display,
        {
            self.on_configuration_change = value.try_into().map_err(|e| {
                format!("error converting supplied value for on_configuration_change: {e}")
            });
            self
        }
        pub fn on_schema_change<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::OnSchemaChange>>,
            T::Error: ::std::fmt::Display,
        {
            self.on_schema_change = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for on_schema_change: {e}"));
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
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Schema>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
        pub fn secure<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.secure = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for secure: {e}"));
            self
        }
        pub fn snowflake_warehouse<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::SnowflakeWarehouse>>,
            T::Error: ::std::fmt::Display,
        {
            self.snowflake_warehouse = value.try_into().map_err(|e| {
                format!("error converting supplied value for snowflake_warehouse: {e}")
            });
            self
        }
        pub fn sql_header<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::SqlHeader>>,
            T::Error: ::std::fmt::Display,
        {
            self.sql_header = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for sql_header: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Tags>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn target_lag<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TargetLag>>,
            T::Error: ::std::fmt::Display,
        {
            self.target_lag = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for target_lag: {e}"));
            self
        }
        pub fn tblproperties<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Tblproperties>>,
            T::Error: ::std::fmt::Display,
        {
            self.tblproperties = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tblproperties: {e}"));
            self
        }
        pub fn transient<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.transient = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transient: {e}"));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<::std::string::String, super::ModelConfigsExtraValue>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ModelConfigs> for super::ModelConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ModelConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                access: value.access?,
                alias: value.alias?,
                auto_refresh: value.auto_refresh?,
                backup: value.backup?,
                bind: value.bind?,
                contract: value.contract?,
                copy_grants: value.copy_grants?,
                database: value.database?,
                docs: value.docs?,
                enabled: value.enabled?,
                file_format: value.file_format?,
                full_refresh: value.full_refresh?,
                grant_access_to: value.grant_access_to?,
                grants: value.grants?,
                group: value.group?,
                hours_to_expiration: value.hours_to_expiration?,
                incremental_strategy: value.incremental_strategy?,
                kms_key_name: value.kms_key_name?,
                labels: value.labels?,
                location: value.location?,
                materialized: value.materialized?,
                meta: value.meta?,
                on_configuration_change: value.on_configuration_change?,
                on_schema_change: value.on_schema_change?,
                persist_docs: value.persist_docs?,
                post_hook: value.post_hook?,
                pre_hook: value.pre_hook?,
                schema: value.schema?,
                secure: value.secure?,
                snowflake_warehouse: value.snowflake_warehouse?,
                sql_header: value.sql_header?,
                tags: value.tags?,
                target_lag: value.target_lag?,
                tblproperties: value.tblproperties?,
                transient: value.transient?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::ModelConfigs> for ModelConfigs {
        fn from(value: super::ModelConfigs) -> Self {
            Self {
                access: Ok(value.access),
                alias: Ok(value.alias),
                auto_refresh: Ok(value.auto_refresh),
                backup: Ok(value.backup),
                bind: Ok(value.bind),
                contract: Ok(value.contract),
                copy_grants: Ok(value.copy_grants),
                database: Ok(value.database),
                docs: Ok(value.docs),
                enabled: Ok(value.enabled),
                file_format: Ok(value.file_format),
                full_refresh: Ok(value.full_refresh),
                grant_access_to: Ok(value.grant_access_to),
                grants: Ok(value.grants),
                group: Ok(value.group),
                hours_to_expiration: Ok(value.hours_to_expiration),
                incremental_strategy: Ok(value.incremental_strategy),
                kms_key_name: Ok(value.kms_key_name),
                labels: Ok(value.labels),
                location: Ok(value.location),
                materialized: Ok(value.materialized),
                meta: Ok(value.meta),
                on_configuration_change: Ok(value.on_configuration_change),
                on_schema_change: Ok(value.on_schema_change),
                persist_docs: Ok(value.persist_docs),
                post_hook: Ok(value.post_hook),
                pre_hook: Ok(value.pre_hook),
                schema: Ok(value.schema),
                secure: Ok(value.secure),
                snowflake_warehouse: Ok(value.snowflake_warehouse),
                sql_header: Ok(value.sql_header),
                tags: Ok(value.tags),
                target_lag: Ok(value.target_lag),
                tblproperties: Ok(value.tblproperties),
                transient: Ok(value.transient),
                extra: Ok(value.extra),
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
    pub struct SeedConfigs {
        column_types:
            ::std::result::Result<::std::option::Option<super::ColumnTypes>, ::std::string::String>,
        copy_grants: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        database:
            ::std::result::Result<::std::option::Option<super::Database>, ::std::string::String>,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        full_refresh: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        grants: ::std::result::Result<::std::option::Option<super::Grants>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        meta: ::std::result::Result<::std::option::Option<super::Meta>, ::std::string::String>,
        persist_docs: ::std::result::Result<
            ::std::option::Option<super::PersistDocsConfig>,
            ::std::string::String,
        >,
        post_hook: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        pre_hook: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        quote_columns: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        schema: ::std::result::Result<::std::option::Option<super::Schema>, ::std::string::String>,
        snowflake_warehouse: ::std::result::Result<
            ::std::option::Option<super::SnowflakeWarehouse>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<::std::option::Option<super::Tags>, ::std::string::String>,
        transient: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        extra: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::SeedConfigsExtraValue>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SeedConfigs {
        fn default() -> Self {
            Self {
                column_types: Ok(Default::default()),
                copy_grants: Ok(Default::default()),
                database: Ok(Default::default()),
                docs: Ok(Default::default()),
                enabled: Ok(Default::default()),
                full_refresh: Ok(Default::default()),
                grants: Ok(Default::default()),
                group: Ok(Default::default()),
                meta: Ok(Default::default()),
                persist_docs: Ok(Default::default()),
                post_hook: Ok(Default::default()),
                pre_hook: Ok(Default::default()),
                quote_columns: Ok(Default::default()),
                schema: Ok(Default::default()),
                snowflake_warehouse: Ok(Default::default()),
                tags: Ok(Default::default()),
                transient: Ok(Default::default()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl SeedConfigs {
        pub fn column_types<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ColumnTypes>>,
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
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Database>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
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
        pub fn full_refresh<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.full_refresh = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for full_refresh: {e}"));
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
            T: ::std::convert::TryInto<::std::option::Option<super::Meta>>,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
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
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
            T::Error: ::std::fmt::Display,
        {
            self.post_hook = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for post_hook: {e}"));
            self
        }
        pub fn pre_hook<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::StringOrArrayOfStrings>>,
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
        pub fn schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Schema>>,
            T::Error: ::std::fmt::Display,
        {
            self.schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schema: {e}"));
            self
        }
        pub fn snowflake_warehouse<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::SnowflakeWarehouse>>,
            T::Error: ::std::fmt::Display,
        {
            self.snowflake_warehouse = value.try_into().map_err(|e| {
                format!("error converting supplied value for snowflake_warehouse: {e}")
            });
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Tags>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn transient<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.transient = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transient: {e}"));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<::std::string::String, super::SeedConfigsExtraValue>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SeedConfigs> for super::SeedConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SeedConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                column_types: value.column_types?,
                copy_grants: value.copy_grants?,
                database: value.database?,
                docs: value.docs?,
                enabled: value.enabled?,
                full_refresh: value.full_refresh?,
                grants: value.grants?,
                group: value.group?,
                meta: value.meta?,
                persist_docs: value.persist_docs?,
                post_hook: value.post_hook?,
                pre_hook: value.pre_hook?,
                quote_columns: value.quote_columns?,
                schema: value.schema?,
                snowflake_warehouse: value.snowflake_warehouse?,
                tags: value.tags?,
                transient: value.transient?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::SeedConfigs> for SeedConfigs {
        fn from(value: super::SeedConfigs) -> Self {
            Self {
                column_types: Ok(value.column_types),
                copy_grants: Ok(value.copy_grants),
                database: Ok(value.database),
                docs: Ok(value.docs),
                enabled: Ok(value.enabled),
                full_refresh: Ok(value.full_refresh),
                grants: Ok(value.grants),
                group: Ok(value.group),
                meta: Ok(value.meta),
                persist_docs: Ok(value.persist_docs),
                post_hook: Ok(value.post_hook),
                pre_hook: Ok(value.pre_hook),
                quote_columns: Ok(value.quote_columns),
                schema: Ok(value.schema),
                snowflake_warehouse: Ok(value.snowflake_warehouse),
                tags: Ok(value.tags),
                transient: Ok(value.transient),
                extra: Ok(value.extra),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SemanticModelConfigs {
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        meta: ::std::result::Result<::std::option::Option<super::Meta>, ::std::string::String>,
        extra: ::std::result::Result<
            ::std::collections::HashMap<
                ::std::string::String,
                super::SemanticModelConfigsExtraValue,
            >,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SemanticModelConfigs {
        fn default() -> Self {
            Self {
                enabled: Ok(Default::default()),
                group: Ok(Default::default()),
                meta: Ok(Default::default()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl SemanticModelConfigs {
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
            T: ::std::convert::TryInto<::std::option::Option<super::Meta>>,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<
                    ::std::string::String,
                    super::SemanticModelConfigsExtraValue,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SemanticModelConfigs> for super::SemanticModelConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SemanticModelConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                enabled: value.enabled?,
                group: value.group?,
                meta: value.meta?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::SemanticModelConfigs> for SemanticModelConfigs {
        fn from(value: super::SemanticModelConfigs) -> Self {
            Self {
                enabled: Ok(value.enabled),
                group: Ok(value.group),
                meta: Ok(value.meta),
                extra: Ok(value.extra),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SnapshotConfigs {
        alias: ::std::result::Result<::std::option::Option<super::Alias>, ::std::string::String>,
        check_cols: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        database:
            ::std::result::Result<::std::option::Option<super::Database>, ::std::string::String>,
        docs:
            ::std::result::Result<::std::option::Option<super::DocsConfig>, ::std::string::String>,
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        grants: ::std::result::Result<::std::option::Option<super::Grants>, ::std::string::String>,
        group: ::std::result::Result<::std::option::Option<super::Group>, ::std::string::String>,
        invalidate_hard_deletes: ::std::result::Result<
            ::std::option::Option<super::InvalidateHardDeletes>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<::std::option::Option<super::Meta>, ::std::string::String>,
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
        strategy:
            ::std::result::Result<::std::option::Option<super::Strategy>, ::std::string::String>,
        tags: ::std::result::Result<::std::option::Option<super::Tags>, ::std::string::String>,
        target_database: ::std::result::Result<
            ::std::option::Option<super::TargetDatabase>,
            ::std::string::String,
        >,
        target_schema: ::std::result::Result<
            ::std::option::Option<super::TargetSchema>,
            ::std::string::String,
        >,
        transient: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        unique_key: ::std::result::Result<
            ::std::option::Option<super::StringOrArrayOfStrings>,
            ::std::string::String,
        >,
        updated_at:
            ::std::result::Result<::std::option::Option<super::UpdatedAt>, ::std::string::String>,
        extra: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::SnapshotConfigsExtraValue>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SnapshotConfigs {
        fn default() -> Self {
            Self {
                alias: Ok(Default::default()),
                check_cols: Ok(Default::default()),
                database: Ok(Default::default()),
                docs: Ok(Default::default()),
                enabled: Ok(Default::default()),
                grants: Ok(Default::default()),
                group: Ok(Default::default()),
                invalidate_hard_deletes: Ok(Default::default()),
                meta: Ok(Default::default()),
                persist_docs: Ok(Default::default()),
                post_hook: Ok(Default::default()),
                pre_hook: Ok(Default::default()),
                quote_columns: Ok(Default::default()),
                strategy: Ok(Default::default()),
                tags: Ok(Default::default()),
                target_database: Ok(Default::default()),
                target_schema: Ok(Default::default()),
                transient: Ok(Default::default()),
                unique_key: Ok(Default::default()),
                updated_at: Ok(Default::default()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl SnapshotConfigs {
        pub fn alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Alias>>,
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
        pub fn database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Database>>,
            T::Error: ::std::fmt::Display,
        {
            self.database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for database: {e}"));
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
        pub fn invalidate_hard_deletes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::InvalidateHardDeletes>>,
            T::Error: ::std::fmt::Display,
        {
            self.invalidate_hard_deletes = value.try_into().map_err(|e| {
                format!("error converting supplied value for invalidate_hard_deletes: {e}")
            });
            self
        }
        pub fn meta<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Meta>>,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
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
            T: ::std::convert::TryInto<::std::option::Option<super::Strategy>>,
            T::Error: ::std::fmt::Display,
        {
            self.strategy = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for strategy: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Tags>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn target_database<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TargetDatabase>>,
            T::Error: ::std::fmt::Display,
        {
            self.target_database = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for target_database: {e}"));
            self
        }
        pub fn target_schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TargetSchema>>,
            T::Error: ::std::fmt::Display,
        {
            self.target_schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for target_schema: {e}"));
            self
        }
        pub fn transient<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BooleanOrJinjaString>>,
            T::Error: ::std::fmt::Display,
        {
            self.transient = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transient: {e}"));
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
            T: ::std::convert::TryInto<::std::option::Option<super::UpdatedAt>>,
            T::Error: ::std::fmt::Display,
        {
            self.updated_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for updated_at: {e}"));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<
                    ::std::string::String,
                    super::SnapshotConfigsExtraValue,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SnapshotConfigs> for super::SnapshotConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SnapshotConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                alias: value.alias?,
                check_cols: value.check_cols?,
                database: value.database?,
                docs: value.docs?,
                enabled: value.enabled?,
                grants: value.grants?,
                group: value.group?,
                invalidate_hard_deletes: value.invalidate_hard_deletes?,
                meta: value.meta?,
                persist_docs: value.persist_docs?,
                post_hook: value.post_hook?,
                pre_hook: value.pre_hook?,
                quote_columns: value.quote_columns?,
                strategy: value.strategy?,
                tags: value.tags?,
                target_database: value.target_database?,
                target_schema: value.target_schema?,
                transient: value.transient?,
                unique_key: value.unique_key?,
                updated_at: value.updated_at?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::SnapshotConfigs> for SnapshotConfigs {
        fn from(value: super::SnapshotConfigs) -> Self {
            Self {
                alias: Ok(value.alias),
                check_cols: Ok(value.check_cols),
                database: Ok(value.database),
                docs: Ok(value.docs),
                enabled: Ok(value.enabled),
                grants: Ok(value.grants),
                group: Ok(value.group),
                invalidate_hard_deletes: Ok(value.invalidate_hard_deletes),
                meta: Ok(value.meta),
                persist_docs: Ok(value.persist_docs),
                post_hook: Ok(value.post_hook),
                pre_hook: Ok(value.pre_hook),
                quote_columns: Ok(value.quote_columns),
                strategy: Ok(value.strategy),
                tags: Ok(value.tags),
                target_database: Ok(value.target_database),
                target_schema: Ok(value.target_schema),
                transient: Ok(value.transient),
                unique_key: Ok(value.unique_key),
                updated_at: Ok(value.updated_at),
                extra: Ok(value.extra),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SourceConfigs {
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<::std::option::Option<super::Meta>, ::std::string::String>,
        tags: ::std::result::Result<::std::option::Option<super::Tags>, ::std::string::String>,
        extra: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::SourceConfigsExtraValue>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SourceConfigs {
        fn default() -> Self {
            Self {
                enabled: Ok(Default::default()),
                meta: Ok(Default::default()),
                tags: Ok(Default::default()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl SourceConfigs {
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
            T: ::std::convert::TryInto<::std::option::Option<super::Meta>>,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Tags>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<::std::string::String, super::SourceConfigsExtraValue>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SourceConfigs> for super::SourceConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SourceConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                enabled: value.enabled?,
                meta: value.meta?,
                tags: value.tags?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::SourceConfigs> for SourceConfigs {
        fn from(value: super::SourceConfigs) -> Self {
            Self {
                enabled: Ok(value.enabled),
                meta: Ok(value.meta),
                tags: Ok(value.tags),
                extra: Ok(value.extra),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct UnitTestConfigs {
        enabled: ::std::result::Result<
            ::std::option::Option<super::BooleanOrJinjaString>,
            ::std::string::String,
        >,
        meta: ::std::result::Result<::std::option::Option<super::Meta>, ::std::string::String>,
        tags: ::std::result::Result<::std::option::Option<super::Tags>, ::std::string::String>,
        extra: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::UnitTestConfigsExtraValue>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for UnitTestConfigs {
        fn default() -> Self {
            Self {
                enabled: Ok(Default::default()),
                meta: Ok(Default::default()),
                tags: Ok(Default::default()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl UnitTestConfigs {
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
            T: ::std::convert::TryInto<::std::option::Option<super::Meta>>,
            T::Error: ::std::fmt::Display,
        {
            self.meta = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for meta: {e}"));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Tags>>,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {e}"));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<
                    ::std::string::String,
                    super::UnitTestConfigsExtraValue,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<UnitTestConfigs> for super::UnitTestConfigs {
        type Error = super::error::ConversionError;
        fn try_from(
            value: UnitTestConfigs,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                enabled: value.enabled?,
                meta: value.meta?,
                tags: value.tags?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::UnitTestConfigs> for UnitTestConfigs {
        fn from(value: super::UnitTestConfigs) -> Self {
            Self {
                enabled: Ok(value.enabled),
                meta: Ok(value.meta),
                tags: Ok(value.tags),
                extra: Ok(value.extra),
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
