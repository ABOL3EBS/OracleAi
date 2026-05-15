use tree_sitter::Language;

pub enum SupportedLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
}

impl SupportedLanguage {
    pub fn from_string(lang: &str) -> Option<Self> {
        match lang {
            "Rust" => Some(Self::Rust),
            "Python" => Some(Self::Python),
            "JavaScript" | "JavaScript (React)" => Some(Self::JavaScript),
            "TypeScript" => Some(Self::TypeScript),
            "TypeScript (React)" => Some(Self::Tsx),
            _ => None,
        }
    }

    pub fn get_tree_sitter_language(&self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::language(),
            Self::Python => tree_sitter_python::language(),
            Self::JavaScript => tree_sitter_javascript::language(),
            Self::TypeScript => tree_sitter_typescript::language_typescript(),
            Self::Tsx => tree_sitter_typescript::language_tsx(),
        }
    }
}
