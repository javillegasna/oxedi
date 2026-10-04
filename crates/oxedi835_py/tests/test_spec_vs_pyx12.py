"""The spec cross-check script against a tiny synthetic map, and against the
real pyx12 maps when pyx12 is installed."""

import importlib.util
import json
import sys

import pytest

from conftest import CORE_TESTS

REPO = CORE_TESTS.parents[2]
SCRIPT = REPO / "scripts" / "spec_vs_pyx12.py"


def load_script():
    spec = importlib.util.spec_from_file_location("spec_vs_pyx12", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    # Dataclasses resolve their annotations through the module registry.
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


MAP = """<?xml version="1.0"?>
<transaction xid="T">
  <loop xid="ISA_LOOP" type="explicit">
    <segment xid="ISA"><usage>R</usage>
      <element xid="ISA01"><data_ele>I01</data_ele><name>Qualifier</name><usage>R</usage>
        <seq>01</seq><valid_codes><code>00</code><code>03</code></valid_codes></element>
    </segment>
    <loop xid="2100">
      <segment xid="CLP"><usage>R</usage>
        <element xid="CLP01"><data_ele>1028</data_ele><name>Claim Id</name><usage>R</usage>
          <seq>01</seq></element>
      </segment>
      <segment xid="REF"><usage>S</usage>
        <element xid="REF01"><data_ele>128</data_ele><name>Qualifier</name><usage>R</usage>
          <seq>01</seq><valid_codes><code>EA</code></valid_codes></element>
        <element xid="REF02"><data_ele>127</data_ele><name>Value</name><usage>R</usage>
          <seq>02</seq></element>
      </segment>
      <segment xid="DTM"><usage>S</usage>
        <element xid="DTM01"><data_ele>374</data_ele><name>Date Qualifier</name><usage>R</usage>
          <seq>01</seq><valid_codes><code>232</code></valid_codes></element>
      </segment>
      <loop xid="2110">
        <segment xid="SVC"><usage>R</usage>
          <element xid="SVC01"><data_ele>782</data_ele><name>Charge</name><usage>R</usage>
            <seq>01</seq></element>
        </segment>
        <segment xid="REF"><usage>S</usage>
          <element xid="REF01"><data_ele>128</data_ele><name>Qualifier</name><usage>R</usage>
            <seq>01</seq><valid_codes><code>6R</code></valid_codes></element>
          <element xid="REF02"><data_ele>127</data_ele><name>Value</name><usage>S</usage>
            <seq>02</seq></element>
        </segment>
        <segment xid="DTM"><usage>S</usage>
          <element xid="DTM01"><data_ele>374</data_ele><name>Date Qualifier</name>
            <usage>R</usage><seq>01</seq><valid_codes /></element>
        </segment>
      </loop>
    </loop>
    <segment xid="IEA"><usage>R</usage></segment>
  </loop>
</transaction>
"""

DATAELE = """<?xml version="1.0"?>
<data_elements>
  <data_ele ele_num="I01" data_type="ID" min_len="2" max_len="2" name="I01"/>
  <data_ele ele_num="1028" data_type="AN" min_len="1" max_len="38" name="Claim"/>
  <data_ele ele_num="128" data_type="ID" min_len="2" max_len="3" name="Qualifier"/>
  <data_ele ele_num="127" data_type="AN" min_len="1" max_len="50" name="Value"/>
  <data_ele ele_num="374" data_type="ID" min_len="3" max_len="3" name="Date"/>
  <data_ele ele_num="782" data_type="R" min_len="1" max_len="18" name="Amount"/>
</data_elements>
"""

CODES = """<?xml version="1.0"?>
<codesets><codeset><id>states</id><version><code>AL</code></version></codeset></codesets>
"""

SPEC = {
    "name": "tiny",
    "loops": {
        "interchange": {"trigger": {"segment": "ISA"}, "end": "IEA"},
        "2100": {"parent": "interchange", "trigger": {"segment": "CLP"},
                 "segments": ["REF"]},
        "2110": {"parent": "2100", "trigger": {"segment": "SVC"}, "segments": ["REF", "DTM"]},
    },
    "segments": {
        "ISA": {"elements": {"1": {"name": "qualifier", "type": "ID", "required": True,
                                   "min": 2, "max": 2, "codes": ["00", "01"]}}},
        "REF": {"elements": {
            "1": {"name": "qualifier", "type": "ID", "required": True, "min": 2, "max": 3},
            "2": {"name": "value", "type": "AN", "required": True, "min": 1, "max": 50},
        }},
        "DTM": {"elements": {"1": {"name": "qualifier", "type": "ID", "required": True,
                                   "min": 3, "max": 3}}},
    },
}


@pytest.fixture
def tiny(tmp_path):
    paths = {}
    for name, text in (("map.xml", MAP), ("dataele.xml", DATAELE), ("codes.xml", CODES)):
        paths[name] = tmp_path / name
        paths[name].write_text(text)
    paths["spec"] = tmp_path / "spec.json"
    paths["spec"].write_text(json.dumps(SPEC))
    paths["ignore"] = tmp_path / "ignore.json"
    paths["ignore"].write_text('{"ignore": []}')
    return paths


def args(tiny, *extra):
    return [
        "--spec", str(tiny["spec"]), "--map", str(tiny["map.xml"]),
        "--dataele", str(tiny["dataele.xml"]), "--codes", str(tiny["codes.xml"]),
        "--ignore", str(tiny["ignore"]), *extra,
    ]


def test_report_and_patch_name_each_difference(tiny, tmp_path):
    script = load_script()
    report, patch = tmp_path / "report.md", tmp_path / "patch.json"
    assert script.main(args(tiny, "--report", str(report), "--patch", str(patch))) == 0
    text = report.read_text()
    draft = json.loads(patch.read_text())
    # REF01 is closed in both places: the spec lacks the union of both lists.
    assert "`codes:REF01` [2100,2110] REF01 (Qualifier) has no code list" in text
    assert draft["segments"]["REF"]["elements"]["1"]["codes"] == ["6R", "EA"]
    # DTM01 is open in 2110 (an empty list), so no list is proposed for it.
    assert "1" not in draft["segments"].get("DTM", {}).get("elements", {})
    # The spec's ISA01 list disagrees with the map's.
    assert "ISA01: codes are ['00', '01'] in the spec, ['00', '03'] in pyx12" in text
    assert draft["segments"]["ISA"]["elements"]["1"]["codes"] == ["00", "03"]
    # REF02 is situational in 2110, so the spec's `required` is stricter.
    assert "REF02: required is True in the spec, False in pyx12" in text
    # The spec's 2100 does not hold DTM; the patch lists the loop's whole segment list.
    assert "`segments:2100:DTM` [2100] loop 2100 does not hold DTM" in text
    assert draft["loops"]["2100"]["segments"] == ["REF", "DTM"]
    # CLP and SVC have no element definitions in the spec.
    assert draft["segments"]["CLP"]["elements"]["1"] == {
        "name": "claim_id", "type": "AN", "required": True, "min": 1, "max": 38,
    }


def test_check_names_loop_element_and_both_values(tiny, capsys):
    script = load_script()
    assert script.main(args(tiny, "--check")) == 1
    err = capsys.readouterr().err
    assert ("codes:ISA01: loop interchange: ISA01: codes are ['00', '01'] in the spec, "
            "['00', '03'] in pyx12") in err
    assert ("usage:REF02:required: loop 2100,2110: REF02: required is True in the spec, "
            "False in pyx12") in err
    # What the spec lacks is reported, never a failure.
    assert "REF01 (Qualifier) has no code list" not in err
    assert "2 disagreement(s)" in err


def test_check_skips_ignored_findings(tiny):
    tiny["ignore"].write_text(json.dumps({"ignore": [
        {"key": "codes:ISA01", "reason": "synthetic"},
        {"key": "usage:REF02:required", "reason": "synthetic"},
    ]}))
    assert load_script().main(args(tiny, "--check")) == 0


def test_ignore_entries_need_a_reason(tiny):
    tiny["ignore"].write_text(json.dumps({"ignore": [{"key": "codes:ISA01"}]}))
    with pytest.raises(SystemExit, match=r"ignore\[0\] needs a non-empty"):
        load_script().main(args(tiny, "--check"))


def test_real_maps_cover_every_loop(tmp_path):
    pytest.importorskip("pyx12")
    script = load_script()
    report, patch = tmp_path / "report.md", tmp_path / "patch.json"
    assert script.main(["--report", str(report), "--patch", str(patch)]) == 0
    text = report.read_text()
    assert "| loops | 16 | 0 | 0 |" in text
    assert "Unmapped pyx12 loops" not in text


@pytest.mark.parametrize("version", ["5010", "4010"])
def test_real_draft_patches_load_over_the_compared_spec(tmp_path, version):
    pytest.importorskip("pyx12")
    from oxedi835 import Spec

    script = load_script()
    report, patch = tmp_path / "report.md", tmp_path / "patch.json"
    assert script.main(["--version", version, "--report", str(report),
                        "--patch", str(patch)]) == 0
    spec = Spec.builtin()
    for name in script.VERSIONS[version]["patches"]:
        spec = spec.patch((script.SPECS / name).read_text())
    # The draft carries code lists, lengths and new definitions in the
    # spec format; the core loads every one of them.
    spec.patch(patch.read_text())
