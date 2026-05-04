#!/usr/bin/env python3
import json
import requests

print(json.dumps({"success": True, "requests_version": requests.__version__}))
