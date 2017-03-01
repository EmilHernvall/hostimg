HostIMG
=======

A work-in-progress daemon for sharing a folder of pictures using a convenient
web UI. The decision to build this as a rust application was mostly based on my
desire to monitor my pictures directory using inotify, and automatically
generating thumbs and scaled copies of each new image.

Current Status: Will index the specified directory during startup, and launch
a web server on port 1080 which will serve images at /gallery. Works quite
well, but could use a lot more polish. inotify-support is still missing, and so
is daemonization.
