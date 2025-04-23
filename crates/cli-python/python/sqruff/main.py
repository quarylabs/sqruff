import sys
from sqruff import run_cli  # Import the Rust extension module's function


def main():
    # Pass command-line arguments to the Rust logic
    try:
        status = run_cli(sys.argv[1:])
        # If the Rust function returns an int (exit code), exit with it
        if isinstance(status, int):
            sys.exit(status)
    except Exception as e:
        # Handle exceptions (if your Rust code raises any Python errors)
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
