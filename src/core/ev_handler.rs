//! Provides the [`handle_event`] function
use std::io::Write;

use super::events::Event;
#[cfg(feature = "search")]
use super::search;
use super::term::cleanup;
use crate::{error::MinusError, input::InputEvent, PagerState};
#[cfg(feature = "search")]
use std::sync::{Arc, Mutex};

/// Respond based on the type of event
///
/// It will mat match the type of event received and based on that, it can take actions like:-
/// - Mutating fields of [`PagerState`]
/// - Handle cleanup and exits
/// - Call search related functions
pub fn handle_event(
    ev: Event,
    mut out: &mut impl Write,
    p: &mut PagerState,
    is_exitted: &mut bool,
    #[cfg(feature = "search")] event_thread_running: &Arc<Mutex<()>>,
) -> Result<(), MinusError> {
    match ev {
        Event::SetData(text) => {
            p.lines = text;
            p.format_lines();
        }
        Event::UserInput(InputEvent::Exit) => {
            p.exit();
            *is_exitted = true;
            cleanup(&mut out, &p.exit_strategy, true)?;
        }
        Event::UserInput(InputEvent::UpdateUpperMark(um)) => p.upper_mark = um,
        Event::UserInput(InputEvent::RestorePrompt) => {
            // Set the message to None and new messages to false as all messages have been shown
            p.message = None;
            p.format_prompt();
        }
        Event::UserInput(InputEvent::UpdateTermArea(c, r)) => {
            p.rows = r;
            p.cols = c;
            // Readjust the text wrapping for the new number of columns
            p.format_lines();
        }
        Event::UserInput(InputEvent::UpdateLineNumber(l)) => {
            p.line_numbers = l;
            p.format_lines();
        }
        #[cfg(feature = "search")]
        Event::UserInput(InputEvent::Search(m)) => {
            p.search_mode = m;
            // Pause the main user input thread from running
            let ilock = event_thread_running.lock().unwrap();
            //            event_thread_running.swap(false, Ordering::SeqCst);
            // Get the query
            let string = search::fetch_input(&mut out, p.search_mode, p.rows)?;
            // Continue the user input thread
            drop(ilock);

            if !string.is_empty() {
                let regex = regex::Regex::new(&string);
                if let Ok(r) = regex {
                    p.search_term = Some(r);

                    // Format the lines, this will automatically generate the PagerState.search_idx
                    p.format_lines();

                    // Move to next search match after the current upper_mark
                    search::next_match(p);
                } else {
                    // Send invalid regex message at the prompt if invalid regex is given
                    p.message = Some("Invalid regular expression. Press Enter".to_owned());
                    p.format_lines();
                }
            }
        }
        #[cfg(feature = "search")]
        Event::UserInput(InputEvent::NextMatch) if p.search_term.is_some() => {
            // If s_mark is less than the length of pager.search_idx
            // and it is not page where the last match is present
            // then increment the s_mark
            if p.search_mark < p.search_idx.len().saturating_sub(1)
                && p.upper_mark + p.rows < p.num_lines()
            {
                p.search_mark += 1;
            }
            // Go to the next match
            search::next_match(p);
        }
        #[cfg(feature = "search")]
        Event::UserInput(InputEvent::PrevMatch) if p.search_term.is_some() => {
            // If no matches, return immediately
            if p.search_idx.is_empty() {
                return Ok(());
            }
            // Decrement the s_mark and get the preceeding index
            p.search_mark = p.search_mark.saturating_sub(1);
            let y = *p.search_idx.iter().nth(p.search_mark).unwrap();
            // If the index is less than or equal to the upper_mark, then set y to the new upper_mark
            if y < p.upper_mark {
                p.upper_mark = y;
                p.format_prompt();
            }
        }
        Event::AppendData(text) => p.append_str(&text),
        Event::SetPrompt(prompt) => {
            p.prompt = prompt;
            p.format_prompt();
        },
        Event::SendMessage(message) => {
            p.message = Some(message);
            p.format_prompt();
        }
        Event::SetLineNumbers(ln) => {
            p.line_numbers = ln;
            p.format_lines();
        }
        Event::SetExitStrategy(es) => p.exit_strategy = es,
        #[cfg(feature = "static_output")]
        Event::SetRunNoOverflow(val) => p.run_no_overflow = val,
        Event::SetInputClassifier(clf) => p.input_classifier = clf,
        Event::AddExitCallback(cb) => p.exit_callbacks.push(cb),
        Event::UserInput(_) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "search")]
    use std::sync::{Arc, Mutex};

    use super::super::events::Event;
    use crate::{ExitStrategy, PagerState};

    use super::handle_event;
    const TEST_STR: &str = "This is some sample text";

    // Tests for event emitting functions of Pager
    #[test]
    fn set_data() {
        let mut ps = PagerState::new().unwrap();
        let ev = Event::SetData(TEST_STR.to_string());
        let mut out = Vec::new();
        #[cfg(feature = "search")]
        let etr = Arc::new(Mutex::new(()));

        handle_event(
            ev,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        assert_eq!(ps.formatted_lines, vec![TEST_STR.to_string()]);
    }

    #[test]
    fn append_str() {
        let mut ps = PagerState::new().unwrap();
        let ev1 = Event::AppendData(format!("{}\n", TEST_STR));
        let ev2 = Event::AppendData(TEST_STR.to_string());
        let mut out = Vec::new();
        #[cfg(feature = "search")]
        let etr = Arc::new(Mutex::new(()));

        handle_event(
            ev1,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        handle_event(
            ev2,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        assert_eq!(
            ps.formatted_lines,
            vec![TEST_STR.to_string(), TEST_STR.to_string()]
        );
    }

    #[test]
    fn set_prompt() {
        let mut ps = PagerState::new().unwrap();
        let ev = Event::SetPrompt(TEST_STR.to_string());
        let mut out = Vec::new();
        #[cfg(feature = "search")]
        let etr = Arc::new(Mutex::new(()));

        handle_event(
            ev,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        assert_eq!(ps.prompt, TEST_STR.to_string());
    }

    #[test]
    fn send_message() {
        let mut ps = PagerState::new().unwrap();
        let ev = Event::SendMessage(TEST_STR.to_string());
        let mut out = Vec::new();
        #[cfg(feature = "search")]
        let etr = Arc::new(Mutex::new(()));

        handle_event(
            ev,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        assert_eq!(ps.message.unwrap(), TEST_STR.to_string());
    }

    #[test]
    #[cfg(feature = "static_output")]
    fn set_run_no_overflow() {
        let mut ps = PagerState::new().unwrap();
        let ev = Event::SetRunNoOverflow(false);
        let mut out = Vec::new();
        #[cfg(feature = "search")]
        let etr = Arc::new(Mutex::new(()));

        handle_event(
            ev,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        assert!(!ps.run_no_overflow);
    }

    #[test]
    fn set_exit_strategy() {
        let mut ps = PagerState::new().unwrap();
        let ev = Event::SetExitStrategy(ExitStrategy::PagerQuit);
        let mut out = Vec::new();
        #[cfg(feature = "search")]
        let etr = Arc::new(Mutex::new(()));

        handle_event(
            ev,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        assert_eq!(ps.exit_strategy, ExitStrategy::PagerQuit);
    }

    #[test]
    fn add_exit_callback() {
        let mut ps = PagerState::new().unwrap();
        let ev = Event::AddExitCallback(Box::new(|| println!("Hello World")));
        let mut out = Vec::new();
        #[cfg(feature = "search")]
        let etr = Arc::new(Mutex::new(()));

        handle_event(
            ev,
            &mut out,
            &mut ps,
            &mut false,
            #[cfg(feature = "search")]
            &etr,
        )
        .unwrap();
        assert_eq!(ps.exit_callbacks.len(), 1);
    }
}
