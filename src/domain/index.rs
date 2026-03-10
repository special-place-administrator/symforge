use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LanguageId {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Dart,
    Perl,
    Elixir,
}

impl LanguageId {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "js" | "jsx" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "c" | "h" => Some(Self::C),
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "rb" => Some(Self::Ruby),
            "php" => Some(Self::Php),
            "swift" => Some(Self::Swift),
            "dart" => Some(Self::Dart),
            "pl" | "pm" => Some(Self::Perl),
            "ex" | "exs" => Some(Self::Elixir),
            _ => None,
        }
    }

    pub fn extensions(&self) -> &[&str] {
        match self {
            Self::Rust => &["rs"],
            Self::Python => &["py"],
            Self::JavaScript => &["js", "jsx"],
            Self::TypeScript => &["ts", "tsx"],
            Self::Go => &["go"],
            Self::Java => &["java"],
            Self::C => &["c", "h"],
            Self::Cpp => &["cpp", "cxx", "cc", "hpp", "hxx", "hh"],
            Self::CSharp => &["cs"],
            Self::Ruby => &["rb"],
            Self::Php => &["php"],
            Self::Swift => &["swift"],
            Self::Dart => &["dart"],
            Self::Perl => &["pl", "pm"],
            Self::Elixir => &["ex", "exs"],
        }
    }

    pub fn support_tier(&self) -> SupportTier {
        match self {
            Self::Rust | Self::Python | Self::JavaScript | Self::TypeScript | Self::Go => {
                SupportTier::QualityFocus
            }
            Self::Java => SupportTier::Broader,
            Self::C
            | Self::Cpp
            | Self::CSharp
            | Self::Ruby
            | Self::Php
            | Self::Swift
            | Self::Dart
            | Self::Perl
            | Self::Elixir => SupportTier::Unsupported,
        }
    }
}

impl fmt::Display for LanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::Ruby => "Ruby",
            Self::Php => "PHP",
            Self::Swift => "Swift",
            Self::Dart => "Dart",
            Self::Perl => "Perl",
            Self::Elixir => "Elixir",
        };
        write!(f, "{name}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupportTier {
    QualityFocus,
    Broader,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileProcessingResult {
    pub relative_path: String,
    pub language: LanguageId,
    pub outcome: FileOutcome,
    pub symbols: Vec<SymbolRecord>,
    pub byte_len: u64,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileOutcome {
    Processed,
    PartialParse { warning: String },
    Failed { error: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolRecord {
    pub name: String,
    pub kind: SymbolKind,
    pub depth: u32,
    pub sort_order: u32,
    pub byte_range: (u32, u32),
    pub line_range: (u32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Module,
    Constant,
    Variable,
    Type,
    Trait,
    Impl,
    Other,
}
