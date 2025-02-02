use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    str::FromStr,
};

#[derive(Copy, Clone, Debug)]
enum HuByteUnit {
    KB,
    MB,
    GB,
}

impl FromStr for HuByteUnit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "k" => Ok(Self::KB),
            "m" => Ok(Self::MB),
            "g" => Ok(Self::GB),
            _ => Err(()),
        }
    }
}

impl Display for HuByteUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            HuByteUnit::KB => "KB",
            HuByteUnit::MB => "MB",
            HuByteUnit::GB => "GB",
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct HuByte {
    val: u64,
    unit: HuByteUnit,
}

impl HuByte {
    pub fn to_u64(&self) -> u64 {
        self.val
            * match self.unit {
                HuByteUnit::KB => 1024,
                HuByteUnit::MB => 1024 * 1024,
                HuByteUnit::GB => 1024 * 1024 * 1024,
            }
    }
}

impl FromStr for HuByte {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();

        let mut digits = String::new();
        let mut unit = String::new();
        while let Some(c) = chars.next() {
            if !c.is_ascii_digit() {
                unit.push(c);
                break;
            }
            digits.push(c);
        }
        unit.extend(chars);

        let digits = digits.parse().map_err(|_| ())?;

        let unit = unit.parse().map_err(|_| ())?;

        Ok(Self { val: digits, unit })
    }
}

impl Display for HuByte {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "{} {} ({})",
            self.val,
            self.unit,
            self.to_u64()
        ))
    }
}
