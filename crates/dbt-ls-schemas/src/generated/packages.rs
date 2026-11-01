#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

#[doc = "Must be in format `org_name/package_name`. Refer to hub.getdbt.com for installation instructions"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PackageIdentifier(::std::string::String);
impl ::std::ops::Deref for PackageIdentifier {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PackageIdentifier> for ::std::string::String {
    fn from(value: PackageIdentifier) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PackageIdentifier {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("^[\\w\\-\\.]+/[\\w\\-\\.]+$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[\\w\\-\\.]+/[\\w\\-\\.]+$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for PackageIdentifier {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PackageIdentifier {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PackageIdentifier {
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
#[doc = "A semantic version string or range, such as [\">=1.0.0\", \"<2.0.0\"]"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum PackageVersion {
    Number(f64),
    String(::std::string::String),
    Array(::std::vec::Vec<::serde_json::Value>),
}
impl ::std::convert::From<f64> for PackageVersion {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}
impl ::std::convert::From<::std::vec::Vec<::serde_json::Value>> for PackageVersion {
    fn from(value: ::std::vec::Vec<::serde_json::Value>) -> Self {
        Self::Array(value)
    }
}
#[doc = "`Packages`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Packages {
    pub packages: ::std::vec::Vec<PackagesPackagesItem>,
}
impl Packages {
    pub fn builder() -> builder::Packages {
        Default::default()
    }
}
#[doc = "`PackagesPackagesItem`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged, deny_unknown_fields)]
pub enum PackagesPackagesItem {
    Variant0 {
        #[doc = "Opt in to prerelease versions of a package"]
        #[serde(
            rename = "install-prerelease",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        install_prerelease: ::std::option::Option<bool>,
        #[doc = "Must be in format `org_name/package_name`. Refer to hub.getdbt.com for installation instructions"]
        package: PackageIdentifier,
        #[doc = "A semantic version string or range, such as [\">=1.0.0\", \"<2.0.0\"]"]
        version: PackageVersion,
    },
    Variant1 {
        #[doc = "Must be in format `org_name/package_name`. Refer to docs.getdbt.com for installation instructions"]
        private: PackageIdentifier,
        #[doc = "Pin your package to a specific release by specifying a release name"]
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        revision: ::std::option::Option<::std::string::String>,
        #[doc = "Subdirectory of the repo where the dbt package is located"]
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        subdirectory: ::std::option::Option<::std::string::String>,
    },
    Variant2 {
        git: ::std::string::String,
        #[doc = "Pin your package to a specific release by specifying a release name"]
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        revision: ::std::option::Option<::std::string::String>,
        #[doc = "Only required if the package is nested in a subdirectory of the git project"]
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        subdirectory: ::std::option::Option<::std::string::String>,
    },
    Variant3 {
        #[doc = "dbt will create a subdirectory in your `packages-install-path` with this name, and the tarball's source code will be installed here."]
        name: ::std::string::String,
        tarball: ::std::string::String,
    },
    Variant4 {
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        local: ::std::option::Option<::std::string::String>,
    },
}
#[doc = " Types for composing complex structures."]
pub mod builder {
    #[derive(Clone, Debug)]
    pub struct Packages {
        packages: ::std::result::Result<
            ::std::vec::Vec<super::PackagesPackagesItem>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for Packages {
        fn default() -> Self {
            Self {
                packages: Err("no value supplied for packages".to_string()),
            }
        }
    }
    impl Packages {
        pub fn packages<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::PackagesPackagesItem>>,
            T::Error: ::std::fmt::Display,
        {
            self.packages = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for packages: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Packages> for super::Packages {
        type Error = super::error::ConversionError;
        fn try_from(value: Packages) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                packages: value.packages?,
            })
        }
    }
    impl ::std::convert::From<super::Packages> for Packages {
        fn from(value: super::Packages) -> Self {
            Self {
                packages: Ok(value.packages),
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
