use std::io::IsTerminal;

use dialoguer::Confirm;

use crate::error::GctlCliError;

/// Asks whether to go ahead, defaulting to no. `yes` answers for the user. With
/// no terminal to ask on, an unanswered question is an error rather than a
/// silent yes or no, so scripts have to say what they mean.
pub fn confirm(question: &str, yes: bool) -> Result<bool, GctlCliError> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        return Err(GctlCliError::Config(
            "this needs confirmation and there is no terminal to ask on; pass --yes to confirm"
                .into(),
        ));
    }
    Confirm::new()
        .with_prompt(question)
        .default(false)
        .interact()
        .map_err(|e| GctlCliError::Config(e.to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn yes_skips_the_question() {
        assert!(confirm("Delete?", true).unwrap());
    }
}
