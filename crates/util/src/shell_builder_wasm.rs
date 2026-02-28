use crate::shell::get_system_shell;
use crate::shell::{Shell, ShellKind};

/// Minimal shell builder for wasm targets.
///
/// Web builds do not execute local processes, but many crates rely on this API
/// for command serialization and labels.
pub struct ShellBuilder {
    program: String,
    args: Vec<String>,
    interactive: bool,
    kind: ShellKind,
}

impl ShellBuilder {
    pub fn new(shell: &Shell, is_windows: bool) -> Self {
        let (program, args) = match shell {
            Shell::System => (get_system_shell(), Vec::new()),
            Shell::Program(shell) => (shell.clone(), Vec::new()),
            Shell::WithArguments { program, args, .. } => (program.clone(), args.clone()),
        };

        Self {
            kind: ShellKind::new(&program, is_windows),
            program,
            args,
            interactive: true,
        }
    }

    pub fn non_interactive(mut self) -> Self {
        self.interactive = false;
        self
    }

    pub fn command_label(&self, command_to_use_in_label: &str) -> String {
        if command_to_use_in_label.trim().is_empty() {
            self.program.clone()
        } else {
            format!("{} -c '{}'", self.program, command_to_use_in_label)
        }
    }

    pub fn redirect_stdin_to_dev_null(self) -> Self {
        self
    }

    pub fn build(
        mut self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> (String, Vec<String>) {
        if let Some(task_command) = task_command {
            let mut combined_command = task_command;
            for argument in task_args {
                combined_command.push(' ');
                combined_command.push_str(&self.kind.to_shell_variable(argument));
            }
            self.args
                .extend(self.kind.args_for_shell(self.interactive, combined_command));
        }

        (self.program, self.args)
    }

    pub fn build_no_quote(
        self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> (String, Vec<String>) {
        self.build(task_command, task_args)
    }

    pub fn build_std_command(
        self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> std::process::Command {
        let (program, args) = self.build(task_command, task_args);
        let mut child = std::process::Command::new(program);
        child.args(args);
        child
    }

    pub fn build_smol_command(
        self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> std::process::Command {
        self.build_std_command(task_command, task_args)
    }

    pub fn kind(&self) -> ShellKind {
        self.kind
    }
}
