# bushel landing-page demo. Re-record per release:
#   demo/setup.sh && vhs demo/bushel-demo.tape && demo/teardown.sh
Output demo/bushel-demo.gif
Output demo/bushel-demo.mp4
Output demo/bushel-demo.webm

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 760
Set Padding 16
Set TypingSpeed 60ms

Type "bushel"
Sleep 500ms
Enter
Sleep 3.5s

# walk the rail: buildkit → cache → comicarr → web → worker
Type "j"
Sleep 600ms
Type "j"
Sleep 600ms
Type "j"
Sleep 600ms
Type "j"
Sleep 900ms

# focus the detail pane on worker, follow its logs
Enter
Sleep 1s
Type "l"
Sleep 1s
Type "F"
Sleep 4.5s

# back to the rail, up to web
Escape
Sleep 800ms
Type "k"
Sleep 800ms

# action menu: stop web (confirmed with the exact command), then start it again
Space
Sleep 1.8s
Type "s"
Sleep 2.2s
Type "y"
Sleep 3s
Type "s"
Sleep 1.5s
Type "y"
Sleep 3.5s

# a glance at images, then home
Type "2"
Sleep 1.8s
Type "1"
Sleep 1.5s
Type "q"
Sleep 800ms
