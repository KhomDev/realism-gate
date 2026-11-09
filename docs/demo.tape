# A short demo of `realism-gate check`.
# Render with https://github.com/charmbracelet/vhs :
#   vhs docs/demo.tape
# The GIF is not committed. The tape is the source.

Output docs/demo.gif
Set Shell bash
Set Width 1100
Set Height 700
Set FontSize 16
Set Padding 16
Set Theme "Catppuccin Mocha"

Hide
Type "cd examples/vault-drain" Enter
Sleep 200ms
Show

Type "realism-gate check --color never --no-declaration" 
Sleep 400ms
Enter
Sleep 3s
