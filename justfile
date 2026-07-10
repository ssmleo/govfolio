# govfolio dev lifecycle recipes.
#
# Admin observability dashboard (goal 091): thin wrappers around
# scripts/dev/admin-panel.ps1, which does the real work (start/stop postgres
# + api + web, PID tracking, orphan-process cleanup by port ownership - see
# that script's header for why port-based killing matters here). Usable
# directly via powershell if `just` isn't installed.

set windows-shell := ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]

# Start postgres + the admin API + the admin web dashboard (idempotent).
admin-run:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/admin-panel.ps1 run

# Stop the admin API + web dashboard, sweeping both ports for orphans. Leaves postgres running.
admin-stop:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/admin-panel.ps1 stop

# Report whether postgres / the admin API / the admin web dashboard are up.
admin-status:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/admin-panel.ps1 status

# Stop local postgres (admin-stop leaves it running; this is the explicit off switch).
pg-stop:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/pg-local.ps1 stop
