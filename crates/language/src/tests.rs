use super::*;

#[test]
fn catalog_recognizes_names_aliases_and_both_slashes_without_providers() {
    let registry = Registry::default();
    for (path, expected) in [
        ("src/app.js", "javascript"),
        ("src\\app.TS", "typescript"),
        ("component.tsx", "tsx"),
        ("component.jsx", "javascript"),
        ("types.cts", "typescript"),
        ("tool.mjs", "javascript"),
        ("C:\\project\\Dockerfile", "dockerfile"),
        ("dir/containerfile", "dockerfile"),
        ("Makefile", "make"),
        ("CMakeLists.txt", "cmake"),
        ("Cargo.lock", "toml"),
        ("uv.lock", "toml"),
        ("BUILD.bazel", "starlark"),
        ("meson.build", "meson"),
        ("foo.csharp", "c-sharp"),
        ("foo.hh", "cpp"),
        ("foo.jsonc", "json"),
        ("foo.svg", "xml"),
        (".bashrc", "bash"),
        ("dir\\.zshrc", "zsh"),
        (".gitattributes", "gitattributes"),
        (".config.toml", "toml"),
        ("file.txt", "text"),
        ("unknown.weird", "text"),
        ("extensionless", "text"),
        (".rs", "text"),
        (".unknown", "text"),
        ("trailing.", "text"),
        ("folder.rs/", "text"),
        ("", "text"),
    ] {
        let selected = registry.resolve(path);
        assert_eq!(selected.language.as_str(), expected, "{path}");
        assert_eq!(selected.provider(Capability::Syntax), None, "{path}");
        assert_eq!(selected.provider(Capability::Semantic), None, "{path}");
    }
    assert_eq!(registry.language("CSharp"), registry.language("c-sharp"));
    assert_eq!(registry.language("RS").unwrap().as_str(), "rust");
    assert_eq!(registry.language("unknown"), None);
    assert_eq!(
        Registry::empty().resolve("file.rs").language.as_str(),
        "text"
    );
}

#[test]
fn overrides_have_documented_precedence_and_case_rules() {
    let mut registry = Registry::default();
    let extension = Association::Extension(".LOCK".into());
    let filename = Association::Filename("CARGO.LOCK".into());
    let path = Association::Path("src\\Cargo.lock".into());
    registry.associate(extension.clone(), "json").unwrap();
    assert_eq!(registry.resolve("Cargo.lock").language.as_str(), "json");
    registry.associate(filename.clone(), "yaml").unwrap();
    assert_eq!(
        registry.resolve("other/cargo.LOCK").language.as_str(),
        "yaml"
    );
    registry.associate(path.clone(), "text").unwrap();
    assert_eq!(registry.resolve("src/Cargo.lock").language.as_str(), "text");
    assert_eq!(registry.resolve("Src/Cargo.lock").language.as_str(), "yaml");
    registry.associate(path.clone(), "RS").unwrap();
    assert_eq!(
        registry.resolve("src\\Cargo.lock").language.as_str(),
        "rust"
    );
    registry.remove_association(path).unwrap();
    assert_eq!(registry.resolve("src/Cargo.lock").language.as_str(), "yaml");
    registry.remove_association(filename).unwrap();
    assert_eq!(registry.resolve("Cargo.lock").language.as_str(), "json");
    registry.remove_association(extension).unwrap();
    assert_eq!(registry.resolve("Cargo.lock").language.as_str(), "toml");

    registry
        .associate(Association::Filename(".rs".into()), "rust")
        .unwrap();
    assert_eq!(registry.resolve(".rs").language.as_str(), "rust");
    assert_eq!(registry.resolve(".RS").language.as_str(), "rust");
}

fn provider(id: &str, priority: i32, capabilities: Vec<Capability>) -> ProviderRegistration {
    ProviderRegistration {
        id: id.into(),
        priority,
        capabilities,
    }
}

#[test]
fn one_registration_selects_each_installed_capability_deterministically() {
    let mut registry = Registry::empty();
    let mut registration = LanguageRegistration::new("example");
    registration.extensions = vec!["ex".into()];
    registration.aliases = vec!["ExampleLang".into()];
    registration.providers = vec![
        provider("low", 0, vec![Capability::Syntax, Capability::Semantic]),
        provider("z-syntax", 1, vec![Capability::Syntax]),
        provider("a-syntax", 1, vec![Capability::Syntax]),
        provider("semantics", 2, vec![Capability::Semantic]),
        provider("no-capability", 100, vec![]),
    ];
    registry.register(registration.clone()).unwrap();
    assert_eq!(
        registry.language("examplelang").unwrap().as_str(),
        "example"
    );
    let selected = registry.resolve("file.ex");
    assert_eq!(selected.provider(Capability::Syntax), Some("a-syntax"));
    assert_eq!(selected.provider(Capability::Semantic), Some("semantics"));
    registration.providers.reverse();
    registry.register(registration.clone()).unwrap();
    let reordered = registry.resolve("file.ex");
    assert_eq!(reordered.syntax, selected.syntax);
    assert_eq!(reordered.semantic, selected.semantic);
    assert_ne!(reordered, selected); // replacement invalidates prior jobs too
    registration.providers.clear();
    registry.register(registration).unwrap();
    let selected = registry.resolve("file.ex");
    assert_eq!(selected.language.as_str(), "example");
    assert_eq!(selected.syntax, None);
    assert_eq!(selected.semantic, None);
}

#[test]
fn invalid_registration_is_atomic_and_cannot_steal_recognition() {
    let mut registry = Registry::default();
    let before = registry.resolve("file.rs");
    let mut conflict = LanguageRegistration::new("other");
    conflict.extensions = vec!["RS".into()];
    assert!(registry.register(conflict).is_err());
    let mut conflict = LanguageRegistration::new("other");
    conflict.aliases = vec!["rust".into()];
    assert!(registry.register(conflict).is_err());
    let mut conflict = LanguageRegistration::new("other");
    conflict.filenames = vec!["DOCKERFILE".into()];
    assert!(registry.register(conflict).is_err());
    assert!(
        registry
            .register(LanguageRegistration::new("MixedCase"))
            .is_err()
    );
    assert!(
        registry
            .associate(Association::Extension("rs".into()), "unknown")
            .is_err()
    );
    for association in [
        Association::Extension(".".into()),
        Association::Extension("tar.gz".into()),
        Association::Filename("dir/name".into()),
        Association::Path("".into()),
    ] {
        assert!(registry.associate(association, "text").is_err());
    }
    let mut duplicate = LanguageRegistration::new("other");
    duplicate.providers = vec![provider("same", 0, vec![]), provider("same", 1, vec![])];
    assert!(registry.register(duplicate).is_err());
    assert_eq!(registry.resolve("file.rs"), before);
    assert_eq!(registry.language("other"), None);
}

#[test]
fn configuration_identity_distinguishes_independent_and_divergent_registries() {
    let original = Registry::default();
    let mut first = original.clone();
    let mut second = original.clone();
    assert_eq!(original.resolve("file.rs"), first.resolve("file.rs"));
    assert_ne!(
        original.resolve("file.rs"),
        Registry::default().resolve("file.rs")
    );
    first
        .associate(Association::Extension("rs".into()), "text")
        .unwrap();
    second
        .associate(Association::Extension("rs".into()), "text")
        .unwrap();
    assert_ne!(first.resolve("file.rs"), second.resolve("file.rs"));
    assert_eq!(original.resolve("file.rs").language.as_str(), "rust");
}
