# Enframe development commands

# Default: show available commands
default:
	@just --list

# Run development server
dev:
	bun run dev

# Build for production
build:
	bun run build

# Run type checking
check:
	bun run check

# Format code with Biome
format:
	bun run format

# Lint code with Biome
lint:
	bun run lint:fix

# Clean build artifacts
clean:
	rm -rf node_modules .svelte-kit src-tauri/target src-tauri/gen build

# Reinstall dependencies
reinstall: clean
	bun install

# Initialize the database with schema
init-db:
	sqlite3 ~/.local/share/com.enframe.app/enframe.db < database.sql

# Stabilize's gate: build, lint and test the crate, run every documented command and compare tracking with its baseline
check-stabilize:
	bash analysis/stabilize/check.sh
