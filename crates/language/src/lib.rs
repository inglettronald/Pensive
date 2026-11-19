//! One portable catalog for language recognition and installed provider selection.
//! No I/O, GPUI, mutable document, parser or semantic engine lives here.

mod catalog;
#[cfg(test)]
mod tests;
mod tokens;

use std::{collections::BTreeMap, fmt, sync::Arc};

pub use tokens::{TokenRole, TokenSpan};

/// A canonical name, stable across registries and independent of source identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageId(String);

impl LanguageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Only broad provider families are defined until their implementations arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Syntax,
    Semantic,
}

/// An installed provider's selection metadata. The host owns its implementation;
/// register only capabilities it actually supplies for this language. No built-in
/// registration currently installs a provider.
#[derive(Clone, Debug)]
pub struct ProviderRegistration {
    pub id: String,
    pub capabilities: Vec<Capability>,
    pub priority: i32,
}

/// Recognition and providers enter through the same registration. Registering an
/// existing canonical ID replaces its entire registration, including providers.
#[derive(Clone, Debug)]
pub struct LanguageRegistration {
    pub id: String,
    pub aliases: Vec<String>,
    pub filenames: Vec<String>,
    pub extensions: Vec<String>,
    pub providers: Vec<ProviderRegistration>,
}

impl LanguageRegistration {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            aliases: Vec::new(),
            filenames: Vec::new(),
            extensions: Vec::new(),
            providers: Vec::new(),
        }
    }
}

/// Explicit associations outrank all bundled recognition, including exact names.
/// Paths are case-sensitive and compare with slash styles unified (no filesystem
/// canonicalization). Filenames and final extensions are ASCII case-insensitive.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Association {
    Path(String),
    Filename(String),
    Extension(String),
}

impl Association {
    fn normalized(self) -> Result<Self, RegistryError> {
        Ok(match self {
            Self::Path(path) if !path.is_empty() => Self::Path(path.replace('\\', "/")),
            Self::Filename(name) if valid_filename(&name) => {
                Self::Filename(name.to_ascii_lowercase())
            }
            Self::Extension(extension) => Self::Extension(normalize_extension(&extension)?),
            _ => return Err(RegistryError("invalid association".into())),
        })
    }
}

/// Includes an opaque configuration identity. Compare the whole selection along
/// with the source stamp and context when validating a future job/cache result.
/// Cloned registries share it until either clone changes, then diverge safely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub language: LanguageId,
    pub syntax: Option<String>,
    pub semantic: Option<String>,
    configuration: Configuration,
}

impl Selection {
    pub fn provider(&self, capability: Capability) -> Option<&str> {
        match capability {
            Capability::Syntax => self.syntax.as_deref(),
            Capability::Semantic => self.semantic.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Configuration(Arc<()>);

impl PartialEq for Configuration {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Configuration {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryError(String);

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RegistryError {}

/// A small in-memory catalog. All mutations are validated before publication.
/// There is always a plain-text fallback, even with no installed providers.
#[derive(Clone, Debug)]
pub struct Registry {
    languages: BTreeMap<LanguageId, LanguageRegistration>,
    associations: BTreeMap<Association, LanguageId>,
    configuration: Configuration,
}

impl Default for Registry {
    fn default() -> Self {
        let mut registry = Self::empty();
        for registration in catalog::registrations() {
            registry
                .register(registration)
                .expect("valid bundled catalog");
        }
        registry
    }
}

impl Registry {
    /// Only plain text is recognized, with no providers installed.
    pub fn empty() -> Self {
        Self {
            languages: BTreeMap::from([(
                LanguageId("text".into()),
                LanguageRegistration::new("text"),
            )]),
            associations: BTreeMap::new(),
            configuration: Configuration::default(),
        }
    }

    /// Canonical IDs and registered aliases are ASCII case-insensitive.
    pub fn language(&self, name: &str) -> Option<LanguageId> {
        let name = name.to_ascii_lowercase();
        self.languages.iter().find_map(|(id, registration)| {
            (id.0 == name || registration.aliases.contains(&name)).then(|| id.clone())
        })
    }

    pub fn register(
        &mut self,
        mut registration: LanguageRegistration,
    ) -> Result<(), RegistryError> {
        if registration.id.is_empty()
            || !registration
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(RegistryError(
                "canonical IDs use lowercase ASCII letters, digits and hyphens".into(),
            ));
        }
        for alias in &mut registration.aliases {
            if !valid_filename(alias) {
                return Err(RegistryError("invalid language alias".into()));
            }
            *alias = alias.to_ascii_lowercase();
        }
        for filename in &mut registration.filenames {
            if !valid_filename(filename) {
                return Err(RegistryError("invalid filename".into()));
            }
            *filename = filename.to_ascii_lowercase();
        }
        for extension in &mut registration.extensions {
            *extension = normalize_extension(extension)?;
        }
        for (index, provider) in registration.providers.iter().enumerate() {
            if provider.id.is_empty()
                || registration.providers[..index]
                    .iter()
                    .any(|other| other.id == provider.id)
            {
                return Err(RegistryError(
                    "provider IDs must be nonempty and unique within a language".into(),
                ));
            }
        }
        for existing in self
            .languages
            .values()
            .filter(|other| other.id != registration.id)
        {
            let names = std::iter::once(&registration.id).chain(&registration.aliases);
            if names
                .into_iter()
                .any(|name| name == &existing.id || existing.aliases.contains(name))
                || registration
                    .filenames
                    .iter()
                    .any(|name| existing.filenames.contains(name))
                || registration
                    .extensions
                    .iter()
                    .any(|extension| existing.extensions.contains(extension))
            {
                return Err(RegistryError(format!(
                    "recognition conflicts with {}",
                    existing.id
                )));
            }
        }
        self.languages
            .insert(LanguageId(registration.id.clone()), registration);
        self.configuration = Configuration::default();
        Ok(())
    }

    /// Last explicit association for the same normalized key wins. The target
    /// must be a known canonical ID or alias; use `text` to force plain text.
    pub fn associate(
        &mut self,
        association: Association,
        language: &str,
    ) -> Result<(), RegistryError> {
        let id = self
            .language(language)
            .ok_or_else(|| RegistryError(format!("unknown language: {language}")))?;
        self.associations.insert(association.normalized()?, id);
        self.configuration = Configuration::default();
        Ok(())
    }

    pub fn remove_association(&mut self, association: Association) -> Result<(), RegistryError> {
        if self
            .associations
            .remove(&association.normalized()?)
            .is_some()
        {
            self.configuration = Configuration::default();
        }
        Ok(())
    }

    /// Precedence: explicit path > explicit filename > explicit extension >
    /// registered filename > registered extension > text. Both slash styles work.
    /// A lone dotfile such as `.rs` has no extension; `.config.toml` does.
    pub fn resolve(&self, path: &str) -> Selection {
        let path = path.replace('\\', "/");
        let filename = path
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let extension = filename
            .rsplit_once('.')
            .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
            .map(|(_, extension)| extension);
        let explicit = self
            .associations
            .get(&Association::Path(path))
            .or_else(|| {
                self.associations
                    .get(&Association::Filename(filename.clone()))
            })
            .or_else(|| {
                extension.and_then(|extension| {
                    self.associations
                        .get(&Association::Extension(extension.into()))
                })
            });
        let language = explicit
            .cloned()
            .or_else(|| {
                self.languages
                    .iter()
                    .find(|(_, registration)| registration.filenames.contains(&filename))
                    .map(|(id, _)| id.clone())
            })
            .or_else(|| {
                extension.and_then(|extension| {
                    self.languages
                        .iter()
                        .find(|(_, registration)| {
                            registration
                                .extensions
                                .iter()
                                .any(|value| value == extension)
                        })
                        .map(|(id, _)| id.clone())
                })
            })
            .unwrap_or_else(|| LanguageId("text".into()));
        let registration = &self.languages[&language];
        let provider = |capability| {
            registration
                .providers
                .iter()
                .filter(|provider| provider.capabilities.contains(&capability))
                .min_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)))
                .map(|provider| provider.id.clone())
        };
        Selection {
            syntax: provider(Capability::Syntax),
            semantic: provider(Capability::Semantic),
            language,
            configuration: self.configuration.clone(),
        }
    }
}

fn valid_filename(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\'])
}

fn normalize_extension(extension: &str) -> Result<String, RegistryError> {
    let extension = extension.strip_prefix('.').unwrap_or(extension);
    if !valid_filename(extension) || extension.contains('.') {
        return Err(RegistryError(
            "extensions must be a nonempty final suffix, with an optional leading dot".into(),
        ));
    }
    Ok(extension.to_ascii_lowercase())
}
