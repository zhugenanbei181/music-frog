#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopEntry {
    Product,
    Capture,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesktopEntryParseError {
    UnknownArgument(String),
    UnexpectedExtraArguments,
}

pub fn parse_entry(arguments: &[String]) -> Result<DesktopEntry, DesktopEntryParseError> {
    match arguments {
        [] => Ok(DesktopEntry::Product),
        [argument] if argument == "--capture" => Ok(DesktopEntry::Capture),
        [argument] => Err(DesktopEntryParseError::UnknownArgument(argument.clone())),
        _ => Err(DesktopEntryParseError::UnexpectedExtraArguments),
    }
}
