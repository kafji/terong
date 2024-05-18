This document records outstanding issues and progress status toward their resolutions.


### Losing inputs.

Once in blue moon, input sink will stop producing captured inputs.

Update late 18/05/24: It's... gone?


### Windows' QuickEvent blocking stdio writes.

When running terong-server in Command Prompt (cmd.exe) user can block-select the terminal output, this causes something called QuickEdit where the terminal stop reading stdout and stderr where prints and logs event are written.

Currently this issues is fixed by shifting the log writes to off-goroutine, see `console.Writer`, and not using print functions other than for short debugging session.

However the losing input issues calls for general writes to stdio as I need to debug the C part of the code. In the future I want to disable QuickEdit mode using `SetConsoleMode`.
