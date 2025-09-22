#!/usr/bin/env python3
"""
Test script for Arelle XBRL parsing functionality
"""

import sys
import json
import os
from arelle import Cntlr

def parse_xbrl_with_arelle(xbrl_file_path, output_file_path):
    """
    Parse XBRL file using Arelle and output results to JSON
    """
    try:
        # Fix Arelle's userAppDir issue on macOS
        import os
        import sys

        # Create the proper Arelle directory for macOS
        if sys.platform == "darwin":
            arelle_dir = os.path.expanduser("~/Library/Application Support/Arelle")
            os.makedirs(arelle_dir, exist_ok=True)

        # Create Arelle controller with lenient settings
        ctrl = Cntlr.Cntlr()

        # Configure Arelle to be more lenient with missing taxonomies
        ctrl.modelManager.validate = False  # Disable validation to allow missing taxonomies
        ctrl.modelManager.validateContinuously = False
        ctrl.modelManager.validateCalcLB = False
        ctrl.modelManager.validateInferDecimals = False

        # Load the XBRL instance
        model_xbrl = ctrl.modelManager.load(xbrl_file_path)

        if model_xbrl is None:
            return {"error": "Failed to load XBRL file"}

        # Extract facts
        facts = []
        for fact in model_xbrl.facts:
            fact_data = {
                "concept": str(fact.qname) if fact.qname else None,
                "value": str(fact.xValue) if fact.xValue is not None else None,
                "context_ref": fact.contextID if fact.contextID else None,
                "unit_ref": fact.unitID if fact.unitID else None,
                "decimals": fact.decimals,
                "precision": fact.precision
            }
            facts.append(fact_data)

        # Extract contexts
        contexts = []
        for context in model_xbrl.contexts.values():
            context_data = {
                "id": context.id,
                "entity": {
                    "identifier": context.entityIdentifier[0] if context.entityIdentifier else None,
                    "scheme": context.entityIdentifier[1] if context.entityIdentifier and len(context.entityIdentifier) > 1 else None
                },
                "period": {
                    "start_date": str(context.startDatetime) if context.startDatetime else None,
                    "end_date": str(context.endDatetime) if context.endDatetime else None,
                    "instant": str(context.instantDatetime) if context.instantDatetime else None
                }
            }
            contexts.append(context_data)

        # Extract units
        units = []
        for unit in model_xbrl.units.values():
            unit_data = {
                "id": unit.id,
                "measure": str(unit.measures[0][0]) if unit.measures and unit.measures[0] else None
            }
            units.append(unit_data)

        # Create result
        result = {
            "facts": facts,
            "contexts": contexts,
            "units": units,
            "success": True
        }

        # Write to output file
        with open(output_file_path, 'w') as f:
            json.dump(result, f, indent=2)

        return result

    except Exception as e:
        return {"error": str(e), "success": False}

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python3 test_arelle.py <xbrl_file> <output_file>")
        sys.exit(1)

    xbrl_file = sys.argv[1]
    output_file = sys.argv[2]

    if not os.path.exists(xbrl_file):
        print(f"Error: XBRL file {xbrl_file} does not exist")
        sys.exit(1)

    result = parse_xbrl_with_arelle(xbrl_file, output_file)

    if result.get("success"):
        print(f"Successfully parsed XBRL file. Output written to {output_file}")
        print(f"Found {len(result.get('facts', []))} facts, {len(result.get('contexts', []))} contexts, {len(result.get('units', []))} units")
    else:
        print(f"Error: {result.get('error', 'Unknown error')}")
        sys.exit(1)
