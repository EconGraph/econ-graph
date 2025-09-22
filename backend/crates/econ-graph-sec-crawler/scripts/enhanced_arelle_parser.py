#!/usr/bin/env python3
"""
Enhanced Arelle XBRL parser with DTS support
Uses locally cached taxonomy files for proper XBRL parsing
"""

import sys
import json
import os
import tempfile
import shutil
from pathlib import Path
from arelle import Cntlr
from arelle.ModelManager import ModelManager

def setup_local_taxonomy_cache(instance_file_path, taxonomy_cache_dir):
    """
    Set up local taxonomy cache by copying DTS files to a temporary directory
    that Arelle can use for parsing
    """
    temp_dir = tempfile.mkdtemp(prefix="xbrl_dts_")

    try:
        # Copy the instance file to temp directory
        instance_filename = os.path.basename(instance_file_path)
        temp_instance_path = os.path.join(temp_dir, instance_filename)
        shutil.copy2(instance_file_path, temp_instance_path)

        # Create a mapping of taxonomy files
        taxonomy_mapping = {}

        # Look for common taxonomy files in the cache directory
        if os.path.exists(taxonomy_cache_dir):
            for filename in os.listdir(taxonomy_cache_dir):
                if filename.endswith(('.xsd', '.xml')):
                    # Copy taxonomy file to temp directory
                    source_path = os.path.join(taxonomy_cache_dir, filename)
                    dest_path = os.path.join(temp_dir, filename)
                    shutil.copy2(source_path, dest_path)

                    # Map the original URL to local file
                    if filename.startswith('aapl-'):
                        taxonomy_mapping[f"aapl-{filename.split('-')[1]}.xsd"] = filename
                    elif filename.startswith('cvx-'):
                        taxonomy_mapping[f"cvx-{filename.split('-')[1]}.xsd"] = filename
                    elif 'us-gaap' in filename:
                        taxonomy_mapping["https://xbrl.fasb.org/us-gaap/2024/elts/us-gaap-2024.xsd"] = filename
                    elif 'dei' in filename:
                        taxonomy_mapping["https://xbrl.sec.gov/dei/2024/dei-2024.xsd"] = filename
                    elif 'srt' in filename:
                        taxonomy_mapping["https://xbrl.fasb.org/srt/2024/elts/srt-2024.xsd"] = filename
                    elif 'country' in filename:
                        taxonomy_mapping["https://xbrl.sec.gov/country/2024/country-2024.xsd"] = filename

        return temp_dir, temp_instance_path, taxonomy_mapping

    except Exception as e:
        # Clean up on error
        shutil.rmtree(temp_dir, ignore_errors=True)
        raise e

def parse_xbrl_with_local_dts(xbrl_file_path, taxonomy_cache_dir, output_file_path):
    """
    Parse XBRL file using Arelle with locally cached DTS files
    """
    temp_dir = None

    try:
        # Set up local taxonomy cache
        temp_dir, temp_instance_path, taxonomy_mapping = setup_local_taxonomy_cache(
            xbrl_file_path, taxonomy_cache_dir
        )

        # Fix Arelle's userAppDir issue on macOS
        if sys.platform == "darwin":
            arelle_dir = os.path.expanduser("~/Library/Application Support/Arelle")
            os.makedirs(arelle_dir, exist_ok=True)

        # Create Arelle controller
        ctrl = Cntlr.Cntlr()

        # Configure Arelle with more lenient settings for missing external taxonomies
        ctrl.modelManager.validate = False  # Disable validation to allow missing taxonomies
        ctrl.modelManager.validateContinuously = False
        ctrl.modelManager.validateCalcLB = False
        ctrl.modelManager.validateInferDecimals = False

        # Disable SSL verification for external taxonomy downloads
        ctrl.modelManager.disclosureSystem = None
        ctrl.modelManager.cntlr.webCache.cacheDir = temp_dir

        # Set up SSL context to ignore certificate verification
        import ssl
        ssl_context = ssl.create_default_context()
        ssl_context.check_hostname = False
        ssl_context.verify_mode = ssl.CERT_NONE
        import urllib3
        urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

        # Set the working directory to our temp directory so relative paths work
        original_cwd = os.getcwd()
        os.chdir(temp_dir)

        try:
            # Load the XBRL instance from temp directory
            model_xbrl = ctrl.modelManager.load(temp_instance_path)

            if model_xbrl is None:
                return {"error": "Failed to load XBRL file", "success": False}

            # Extract facts with enhanced taxonomy information
            facts = []
            for fact in model_xbrl.facts:
                fact_data = {
                    "concept": str(fact.qname) if fact.qname else None,
                    "namespace": str(fact.qname.namespaceURI) if fact.qname and hasattr(fact.qname, 'namespaceURI') else None,
                    "local_name": str(fact.qname.localName) if fact.qname and hasattr(fact.qname, 'localName') else None,
                    "value": str(fact.xValue) if fact.xValue is not None else None,
                    "context_ref": fact.contextID if fact.contextID else None,
                    "unit_ref": fact.unitID if fact.unitID else None,
                    "decimals": fact.decimals,
                    "precision": fact.precision,
                    "fact_type": str(type(fact.xValue).__name__) if fact.xValue is not None else None
                }
                facts.append(fact_data)

            # Extract contexts with enhanced period information
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
                        "instant": str(context.instantDatetime) if context.instantDatetime else None,
                        "period_type": "duration" if context.startDatetime and context.endDatetime else "instant"
                    },
                    "scenario": getattr(context, 'scenario', None),
                    "segment": getattr(context, 'segment', None)
                }
                contexts.append(context_data)

            # Extract units with enhanced measure information
            units = []
            for unit in model_xbrl.units.values():
                unit_data = {
                    "id": unit.id,
                    "measure": str(unit.measures[0][0]) if unit.measures and unit.measures[0] else None,
                    "measure_namespace": str(unit.measures[0][0].namespaceURI) if unit.measures and unit.measures[0] and hasattr(unit.measures[0][0], 'namespaceURI') else None,
                    "measure_local_name": str(unit.measures[0][0].localName) if unit.measures and unit.measures[0] and hasattr(unit.measures[0][0], 'localName') else None,
                    "unit_type": "simple" if len(unit.measures) == 1 and len(unit.measures[0]) == 1 else "complex"
                }
                units.append(unit_data)

            # Extract taxonomy information
            taxonomy_concepts = []
            if hasattr(model_xbrl, 'qnameConcepts'):
                for qname, concept in model_xbrl.qnameConcepts.items():
                    try:
                        concept_data = {
                            "qname": str(qname) if qname else None,
                            "namespace": str(qname.namespaceURI) if qname and hasattr(qname, 'namespaceURI') else None,
                            "local_name": str(qname.localName) if qname and hasattr(qname, 'localName') else None,
                            "abstract": bool(getattr(concept, 'abstract', False)) if concept else None,
                            "substitution_group": str(getattr(concept, 'substitutionGroupQname', '')) if concept and hasattr(concept, 'substitutionGroupQname') else None,
                            "type_name": str(getattr(concept, 'typeQname', '')) if concept and hasattr(concept, 'typeQname') else None,
                            "period_type": str(getattr(concept, 'periodType', '')) if concept and hasattr(concept, 'periodType') else None,
                            "balance": str(getattr(concept, 'balance', '')) if concept and hasattr(concept, 'balance') else None,
                            "nillable": bool(getattr(concept, 'nillable', True)) if concept else None
                        }
                        taxonomy_concepts.append(concept_data)
                    except Exception as e:
                        # Skip problematic concepts
                        continue

            # Extract DTS references
            dts_references = []
            if hasattr(model_xbrl, 'document') and hasattr(model_xbrl.document, 'xmlDocument'):
                # Look for schemaRef and linkbaseRef elements
                try:
                    from lxml import etree
                    root = model_xbrl.document.xmlDocument.getroot()

                    # Find schemaRef elements
                    for schema_ref in root.xpath('.//link:schemaRef', namespaces={'link': 'http://www.xbrl.org/2003/linkbase'}):
                        href = schema_ref.get('{http://www.w3.org/1999/xlink}href')
                        if href:
                            dts_references.append({
                                "reference_type": "schemaRef",
                                "href": href,
                                "role": schema_ref.get('{http://www.w3.org/1999/xlink}role'),
                                "arcrole": schema_ref.get('{http://www.w3.org/1999/xlink}arcrole')
                            })

                    # Find linkbaseRef elements
                    for linkbase_ref in root.xpath('.//link:linkbaseRef', namespaces={'link': 'http://www.xbrl.org/2003/linkbase'}):
                        href = linkbase_ref.get('{http://www.w3.org/1999/xlink}href')
                        if href:
                            dts_references.append({
                                "reference_type": "linkbaseRef",
                                "href": href,
                                "role": linkbase_ref.get('{http://www.w3.org/1999/xlink}role'),
                                "arcrole": linkbase_ref.get('{http://www.w3.org/1999/xlink}arcrole')
                            })
                except ImportError:
                    # lxml not available, skip DTS reference extraction
                    pass

            # Create enhanced result
            result = {
                "facts": facts,
                "contexts": contexts,
                "units": units,
                "taxonomy_concepts": taxonomy_concepts,
                "dts_references": dts_references,
                "taxonomy_mapping": taxonomy_mapping,
                "success": True,
                "metadata": {
                    "instance_file": os.path.basename(xbrl_file_path),
                    "temp_directory": temp_dir,
                    "facts_count": len(facts),
                    "contexts_count": len(contexts),
                    "units_count": len(units),
                    "concepts_count": len(taxonomy_concepts),
                    "dts_references_count": len(dts_references)
                }
            }

            # Write to output file
            with open(output_file_path, 'w') as f:
                json.dump(result, f, indent=2)

            return result

        finally:
            # Restore original working directory
            os.chdir(original_cwd)

    except Exception as e:
        import traceback
        error_details = {
            "error": str(e),
            "error_type": type(e).__name__,
            "traceback": traceback.format_exc(),
            "success": False
        }
        return error_details

    finally:
        # Clean up temporary directory
        if temp_dir and os.path.exists(temp_dir):
            shutil.rmtree(temp_dir, ignore_errors=True)

if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("Usage: python3 enhanced_arelle_parser.py <xbrl_file> <taxonomy_cache_dir> <output_file>")
        sys.exit(1)

    xbrl_file = sys.argv[1]
    taxonomy_cache_dir = sys.argv[2]
    output_file = sys.argv[3]

    if not os.path.exists(xbrl_file):
        print(f"Error: XBRL file {xbrl_file} does not exist")
        sys.exit(1)

    if not os.path.exists(taxonomy_cache_dir):
        print(f"Warning: Taxonomy cache directory {taxonomy_cache_dir} does not exist")

    result = parse_xbrl_with_local_dts(xbrl_file, taxonomy_cache_dir, output_file)

    if result.get("success"):
        metadata = result.get("metadata", {})
        print(f"Successfully parsed XBRL file with local DTS support")
        print(f"Output written to {output_file}")
        print(f"Found {metadata.get('facts_count', 0)} facts, {metadata.get('contexts_count', 0)} contexts, {metadata.get('units_count', 0)} units")
        print(f"Found {metadata.get('concepts_count', 0)} taxonomy concepts, {metadata.get('dts_references_count', 0)} DTS references")
    else:
        print(f"Error: {result.get('error', 'Unknown error')}")
        sys.exit(1)
