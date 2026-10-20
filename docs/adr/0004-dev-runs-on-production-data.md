# `make dev` runs on Production data, with backup before migrate

Day-to-day development uses the real data so that it is tested against realistic content; `make sandbox` is the separate, disposable alternative. To make that safe, the database is backed up automatically before any migration that upgrades existing data, and the newest five backups are kept. The Environment is fixed at build time and the app refuses to start if the identifier and Environment disagree, so Sandbox can never touch Production data.
