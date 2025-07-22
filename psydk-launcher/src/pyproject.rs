use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PyProjectToml {
    #[serde(flatten)]
    inner: pyproject_toml::PyProjectToml,
    tool: Option<Tool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Tool {
    maturin: Option<ToolMaturin>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ToolMaturin {
    sdist_include: Option<Vec<String>>,
}

impl std::ops::Deref for PyProjectToml {
    type Target = pyproject_toml::PyProjectToml;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl PyProjectToml {
    pub fn new(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    pub fn name(&self) -> Option<&String> {
        self.project.as_ref().and_then(|p| Some(&p.name))
    }

    pub fn version(&self) -> Option<String> {
        self.project
            .as_ref()
            .and_then(|p| p.version.as_ref())
            .and_then(|v| format!("{}", v).parse::<String>().ok())
    }

    pub fn description(&self) -> Option<&String> {
        self.project.as_ref().and_then(|p| p.description.as_ref())
    }
}
