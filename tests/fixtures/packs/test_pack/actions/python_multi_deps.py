#!/usr/bin/env python3
import json
import requests
import yaml

print(json.dumps({
    "success": True,
    "requests_version": requests.__version__,
    "pyyaml_version": yaml.__version__,
}))
