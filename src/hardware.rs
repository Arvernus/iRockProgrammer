#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareType {
    IRock424,
    IRock212,
    IRock200,
    IRock300,
    IRock400,
}

impl HardwareType {
    pub fn repo(&self) -> &'static str {
        match self {
            HardwareType::IRock424 => "Arvernus/iRock-424",
            HardwareType::IRock212 => "Arvernus/iRock-212",
            HardwareType::IRock200 | HardwareType::IRock300 | HardwareType::IRock400 => {
                "Arvernus/iRock-200-300-400"
            }
        }
    }
    pub fn all() -> &'static [HardwareType] {
        &[
            HardwareType::IRock424,
            HardwareType::IRock212,
            HardwareType::IRock200,
            HardwareType::IRock300,
            HardwareType::IRock400,
        ]
    }
}

impl std::fmt::Display for HardwareType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            HardwareType::IRock424 => "iRock 424",
            HardwareType::IRock212 => "iRock 212",
            HardwareType::IRock200 => "iRock 200",
            HardwareType::IRock300 => "iRock 300",
            HardwareType::IRock400 => "iRock 400",
        };
        write!(f, "{}", s)
    }
}
