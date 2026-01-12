#!/usr/bin/env python3
"""Start the Marginalia web server."""

import os
import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from agents.api import run_server

if __name__ == "__main__":
    host = os.getenv("HOST", "127.0.0.1")
    port = int(os.getenv("PORT", "8000"))

    print(f"Starting Marginalia server at http://{host}:{port}")
    print("Press Ctrl+C to stop")

    run_server(host=host, port=port)
