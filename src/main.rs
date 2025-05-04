use anyhow::Context;
use crossterm::event::{Event, EventStream, KeyCode};
use ratatui::{
    layout::{Layout, Margin},
    style::Style,
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarState},
};
use std::{ffi::OsStr, io::stderr, process::Stdio};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_stream::StreamExt;

pub fn print_usage(arg0: &OsStr) {
    eprintln!("Usage: {} <command> [args]", arg0.to_string_lossy());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveWidget {
    Stdout,
    Stderr,
}

impl ActiveWidget {
    fn switch(&mut self) {
        match self {
            ActiveWidget::Stdout => *self = ActiveWidget::Stderr,
            ActiveWidget::Stderr => *self = ActiveWidget::Stdout,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().collect::<Vec<_>>();
    let arg0 = args.remove(0);

    if args.is_empty() || (args.len() == 1 && args[0] == "-h") {
        print_usage(&arg0);
        return Ok(());
    }

    let cmd = args.remove(0);

    let mut child = tokio::process::Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start process")?;

    let mut child_stdout = child.stdout.take().context("Failed to get stdout")?;
    let mut child_stderr = child.stderr.take().context("Failed to get stderr")?;

    let mut terminal = ratatui::init();

    let mut stdout_buf: Vec<String> = Vec::new();
    let mut stderr_buf: Vec<String> = Vec::new();

    // let mut child_running = true;
    let mut read_stdout = true;
    let mut read_stderr = true;
    // let child_stdout = Some(child_stdout);
    // let child_stderr = Some(child_stderr);

    let mut out_buf = vec![0; 1024];
    let mut err_buf = vec![0; 1024];
    let mut events = EventStream::new();
    let mut stdout_scroll_offset = 0usize;
    let mut stderr_scroll_offset = 0usize;
    let mut stdout_autoscroll = true;
    let mut stderr_autoscroll = true;

    let mut active_widget = ActiveWidget::Stdout;

    loop {
        let scroll_page = if let Ok(size) = terminal.size() {
            (size.height as f32 / 3.0).floor() as usize
        } else {
            10
        };

        tokio::select! {
            x = child_stdout.read(&mut out_buf), if read_stdout => {
                match x {
                    Ok(0) => {
                        read_stdout = false;
                    }
                    Ok(bytes) => {
                        let data = String::from_utf8_lossy(&out_buf[..bytes]).into_owned();
                        stdout_buf.push(data);
                    }

                    _ => {
                        read_stdout = false;
                    },
                }
           }
            x = child_stderr.read(&mut err_buf), if read_stderr => {
                match x {
                    Ok(0) => {
                        read_stderr = false;
                    }
                    Ok(bytes) => {
                        let data = String::from_utf8_lossy(&err_buf[..bytes]).into_owned();
                        stderr_buf.push(data);
                    }
                    _ => {
                        read_stderr = false;
                    },
                }
            }
            x = events.next() => {
                match x {
                    Some(Ok(event)) => {
                        if let Event::Key(key) = event {
                            match key.code {
                                KeyCode::Esc => {
                                    break;
                                }
                                KeyCode::Tab => {
                                    active_widget.switch();
                                }
                                KeyCode::Up => {
                                    if active_widget == ActiveWidget::Stdout {
                                        stdout_scroll_offset = stdout_scroll_offset.saturating_sub(1);
                                        stdout_autoscroll = false;
                                    } else {
                                        stderr_scroll_offset = stderr_scroll_offset.saturating_sub(1);
                                        stderr_autoscroll = false;
                                    }
                                }
                                KeyCode::Down => {
                                    if active_widget == ActiveWidget::Stdout {
                                        stdout_scroll_offset = stdout_scroll_offset.saturating_add(1);
                                        stdout_autoscroll = false;
                                    } else {
                                        stderr_scroll_offset = stderr_scroll_offset.saturating_add(1);
                                        stderr_autoscroll = false;
                                    }
                                }
                                KeyCode::PageUp => {
                                    if active_widget == ActiveWidget::Stdout {
                                        stdout_scroll_offset = stdout_scroll_offset.saturating_sub(scroll_page);
                                        stdout_autoscroll = false;
                                    } else {
                                        stderr_scroll_offset = stderr_scroll_offset.saturating_sub(scroll_page);
                                        stderr_autoscroll = false;
                                    }
                                }
                                KeyCode::PageDown => {
                                    if active_widget == ActiveWidget::Stdout {
                                        stdout_scroll_offset = stdout_scroll_offset.saturating_add(scroll_page);
                                        stdout_autoscroll = false;
                                    } else {
                                        stderr_scroll_offset = stderr_scroll_offset.saturating_add(scroll_page);
                                        stderr_autoscroll = false;
                                    }
                                }
                                KeyCode::Home => {
                                    if active_widget == ActiveWidget::Stdout {
                                        stdout_scroll_offset = 0;
                                        stdout_autoscroll = false;
                                    } else {
                                        stderr_scroll_offset = 0;
                                        stderr_autoscroll = false;
                                    }
                                }
                                KeyCode::End => {
                                    if active_widget == ActiveWidget::Stdout {
                                        stdout_autoscroll = true;
                                    } else {
                                        stderr_autoscroll = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if terminal
            .draw(|frame| {
                let layout = Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .constraints(
                        [
                            ratatui::layout::Constraint::Percentage(50),
                            ratatui::layout::Constraint::Percentage(50),
                        ]
                        .as_ref(),
                    )
                    .split(frame.area());

                // don't let ratatui do the wrapping, we'll do it ourselves with `textwrap`
                let stdout_width = layout[0].width as usize - 2;
                let stderr_width = layout[1].width as usize - 2;
                let height = layout[0].height as usize - 2;

                let all_stdout = stdout_buf.concat();
                let o: Vec<_> = textwrap::wrap(&all_stdout, stdout_width)
                    .into_iter()
                    .map(|line| Line::from(line))
                    .collect();
                let all_stderr = stderr_buf.concat();
                let e: Vec<_> = textwrap::wrap(&all_stderr, stderr_width)
                    .into_iter()
                    .map(|line| Line::from(line))
                    .collect();

                if stdout_scroll_offset + height >= o.len() {
                    stdout_autoscroll = true;
                }
                if stdout_autoscroll {
                    // set a scroll offset so that the last line is always visible
                    stdout_scroll_offset = o.len().saturating_sub(height);
                }

                if stderr_scroll_offset + height >= e.len() {
                    stderr_autoscroll = true;
                }
                if stderr_autoscroll {
                    // set a scroll offset so that the last line is always visible
                    stderr_scroll_offset = e.len().saturating_sub(height);
                }

                let mut stdout_scrollbar_state =
                    ScrollbarState::new(o.len().saturating_sub(height))
                        .position(stdout_scroll_offset);
                let stdout_panel = Paragraph::new(o)
                    .block(
                        Block::new()
                            .title_top("stdout")
                            .title_top(
                                Line::from(if stdout_autoscroll {
                                    "autoscrolling"
                                } else {
                                    ""
                                })
                                .right_aligned(),
                            )
                            .title_top(Line::from(if read_stdout { "" } else { "EOF" }).centered())
                            .borders(Borders::ALL)
                            .border_style(if active_widget == ActiveWidget::Stdout {
                                Style::default().fg(ratatui::style::Color::Green)
                            } else {
                                Style::default()
                            }),
                    )
                    .scroll((stdout_scroll_offset as u16, 0));
                let stdout_scrollbar =
                    Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight);

                let mut stderr_scrollbar_state =
                    ScrollbarState::new(e.len().saturating_sub(height))
                        .position(stderr_scroll_offset);
                let stderr_panel = Paragraph::new(e)
                    .block(
                        Block::new()
                            .title("stderr")
                            .title_top(
                                Line::from(if stderr_autoscroll {
                                    "autoscrolling"
                                } else {
                                    ""
                                })
                                .right_aligned(),
                            )
                            .title_top(Line::from(if read_stderr { "" } else { "EOF" }).centered())
                            .borders(Borders::ALL)
                            .border_style(if active_widget == ActiveWidget::Stderr {
                                Style::default().fg(ratatui::style::Color::Green)
                            } else {
                                Style::default()
                            }),
                    )
                    .scroll((stderr_scroll_offset as u16, 0));
                let stderr_scrollbar =
                    Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight);

                frame.render_widget(stdout_panel, layout[0]);
                frame.render_stateful_widget(
                    stdout_scrollbar,
                    layout[0].inner(Margin {
                        vertical: 1,
                        horizontal: 0,
                    }),
                    &mut stdout_scrollbar_state,
                );
                frame.render_widget(stderr_panel, layout[1]);
                frame.render_stateful_widget(
                    stderr_scrollbar,
                    layout[1].inner(Margin {
                        vertical: 1,
                        horizontal: 0,
                    }),
                    &mut stderr_scrollbar_state,
                );
            })
            .is_err()
        {
            break;
        }
    }

    ratatui::restore();

    Ok(())
}

#[test]
fn test_wrap() {
    //                0        1         2         3         4         5         6         7         8
    //                12345678901234567890
    let text = "hello world\n\n";
    let w = textwrap::wrap(text, 20);
    dbg!(w);
}
