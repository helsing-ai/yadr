# A fixture for yadr's end-to-end tests. Never evaluated; only ever read as text.

# YADR: 2024-04-09 Pin the toolchain in the lock file
#
# In the context of builds that have to reproduce on a colleague's machine and in CI, we
# faced drift between whichever toolchain each of them happened to have installed.
#
# We decided for pinning an exact toolchain version in the lock file, and neglected tracking
# the latest stable release.
#
# We did this to achieve builds that produce the same result everywhere, accepting that
# picking up a new compiler becomes a deliberate change rather than something that happens on
# its own.
#
# We think this is the right trade-off because a build that only fails on someone else's
# machine costs far more to chase down than an occasional version bump.
{ }
