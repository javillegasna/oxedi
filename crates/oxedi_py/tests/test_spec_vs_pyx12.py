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
                 "occurrences": {
                     "clp": {"segment": "CLP", "pos": 0, "usage": "required"},
                     "ref": {"segment": "REF", "pos": 0, "usage": "situational",
                             "codes": {"1": ["EA"]}}}},
        "2110": {"parent": "2100", "trigger": {"segment": "SVC"},
                 "occurrences": {
                     "svc": {"segment": "SVC", "pos": 0, "usage": "required"},
                     "ref": {"segment": "REF", "pos": 0, "usage": "situational",
                             "codes": {"1": ["6R"]}},
                     "dtm": {"segment": "DTM", "pos": 0, "usage": "situational"}}},
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
    # The spec's 2100 does not hold DTM; the patch adds an occurrence for it.
    assert "`segments:2100:DTM` [2100] loop 2100 does not hold DTM" in text
    assert draft["loops"]["2100"]["occurrences"] == {"dtm": {
        "segment": "DTM", "pos": 0, "usage": "situational", "codes": {"1": ["232"]}}}
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
    # The spec's 2100 declares its occurrences, and the map's DTM is not one of them.
    assert ("occurrences:2100:dtm: loop 2100: pyx12 occurrence dtm (DTM '') has no "
            "counterpart in the spec") in err
    # A mapped loop that declares no occurrences fails.
    assert ("occurrences:interchange: loop interchange declares no occurrences; pyx12 places "
            "1 in it") in err
    # What the spec lacks is reported, never a failure.
    assert "REF01 (Qualifier) has no code list" not in err
    assert "4 disagreement(s)" in err


def test_check_skips_ignored_findings(tiny):
    tiny["ignore"].write_text(json.dumps({"ignore": [
        {"key": "codes:ISA01", "reason": "synthetic"},
        {"key": "usage:REF02:required", "reason": "synthetic"},
        {"key": "occurrences:2100:dtm", "reason": "synthetic"},
        {"key": "occurrences:interchange", "reason": "synthetic"},
    ]}))
    assert load_script().main(args(tiny, "--check")) == 0


def test_ignore_entries_need_a_reason(tiny):
    tiny["ignore"].write_text(json.dumps({"ignore": [{"key": "codes:ISA01"}]}))
    with pytest.raises(SystemExit, match=r"ignore\[0\] needs a non-empty"):
        load_script().main(args(tiny, "--check"))


def test_check_reports_what_only_the_spec_defines(tiny, capsys):
    spec = json.loads(tiny["spec"].read_text())
    spec["segments"]["REF"]["elements"]["3"] = {"name": "extra", "type": "AN"}
    spec["segments"]["ZZZ"] = {"elements": {"1": {"name": "z", "type": "AN"}}}
    tiny["spec"].write_text(json.dumps(spec))
    assert load_script().main(args(tiny, "--check")) == 1
    err = capsys.readouterr().err
    assert ("usage:REF03: loop 2100,2110: REF03 is defined in the spec; pyx12 lists no "
            "such position") in err
    assert ("segments:ZZZ: loop -: segment ZZZ is defined in the spec; no mapped pyx12 "
            "loop holds it") in err
    assert "6 disagreement(s)" in err


def test_draft_drops_empty_codes_and_keeps_types_without_a_data_element(tiny, tmp_path):
    tiny["map.xml"].write_text(
        MAP.replace("<code>EA</code>", "<code>EA</code><code/>", 1)
        .replace("<data_ele>127</data_ele><name>Value</name><usage>R</usage>",
                 "<data_ele>9999</data_ele><name>Value</name><usage>R</usage>", 1)
    )
    script = load_script()
    report, patch = tmp_path / "report.md", tmp_path / "patch.json"
    assert script.main(args(tiny, "--report", str(report), "--patch", str(patch))) == 0
    draft = json.loads(patch.read_text())
    ref = draft["segments"]["REF"]["elements"]
    assert ref["1"]["codes"] == ["6R", "EA"]
    # REF02's first place names a data element dataele.xml lacks: its type and
    # lengths are not compared, so the patch never writes null over them.
    assert "type" not in ref.get("2", {})
    assert ("`types:REF02` [2100,2110] REF02: data element '9999' is not in dataele.xml; "
            "type and lengths are not compared") in report.read_text()


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
    from oxedi import Spec

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


@pytest.mark.parametrize("version", ["5010", "4010"])
def test_real_spec_agrees_with_the_map(version, capsys):
    pytest.importorskip("pyx12")
    assert load_script().main(["--check", "--version", version]) == 0, capsys.readouterr().err


OCC_MAP = """<?xml version="1.0"?>
<transaction xid="T">
  <loop xid="ISA_LOOP" type="explicit">
    <repeat>&gt;1</repeat>
    <segment xid="ISA"><name>Interchange Header</name><usage>R</usage><pos>010</pos>
      <max_use>1</max_use>
      <element xid="ISA01"><data_ele>I01</data_ele><name>Qualifier</name><usage>R</usage>
        <seq>01</seq><valid_codes><code>00</code></valid_codes></element>
    </segment>
    <loop xid="2100">
      <repeat>2</repeat>
      <segment xid="CLP"><name>Claim</name><usage>R</usage><pos>020</pos><max_use>1</max_use>
        <element xid="CLP01"><data_ele>1028</data_ele><name>Claim Id</name><usage>R</usage>
          <seq>01</seq></element>
      </segment>
      <segment xid="REF"><name>Other Id</name><usage>S</usage><pos>040</pos><max_use>5</max_use>
        <element xid="REF01"><data_ele>128</data_ele><name>Qualifier</name><usage>R</usage>
          <seq>01</seq><valid_codes><code>EA</code><code>BB</code></valid_codes></element>
        <element xid="REF02"><data_ele>127</data_ele><name>Value</name><usage>R</usage>
          <seq>02</seq></element>
      </segment>
      <segment xid="REF"><name>Provider Id</name><usage>S</usage><pos>040</pos>
        <max_use>&gt;1</max_use>
        <element xid="REF01"><data_ele>128</data_ele><name>Qualifier</name><usage>R</usage>
          <seq>01</seq><valid_codes><code>1A</code></valid_codes></element>
      </segment>
      <segment xid="DTM"><name>Claim Date</name><usage>S</usage><pos>030</pos><max_use>2</max_use>
        <element xid="DTM01"><data_ele>374</data_ele><name>Date Qualifier</name><usage>R</usage>
          <seq>01</seq><valid_codes><code>232</code></valid_codes></element>
      </segment>
      <loop xid="2110">
        <repeat>999</repeat>
        <segment xid="SVC"><name>Service</name><usage>R</usage><pos>070</pos>
          <element xid="SVC01"><data_ele>782</data_ele><name>Charge</name><usage>R</usage>
            <seq>01</seq></element>
        </segment>
        <segment xid="DTM"><name>Service Date</name><usage>S</usage><pos>080</pos>
          <element xid="DTM01"><data_ele>374</data_ele><name>Date Qualifier</name>
            <usage>R</usage><seq>01</seq><valid_codes><code>472</code></valid_codes></element>
        </segment>
      </loop>
    </loop>
    <segment xid="IEA"><usage>R</usage><pos>030</pos></segment>
  </loop>
</transaction>
"""

OCC_SEGMENTS = {
    "ISA": {"elements": {"1": {"name": "qualifier", "type": "ID", "required": True,
                               "min": 2, "max": 2, "codes": ["00"]}}},
    "CLP": {"elements": {"1": {"name": "claim_id", "type": "AN", "required": True,
                               "min": 1, "max": 38}}},
    "REF": {"elements": {
        "1": {"name": "qualifier", "type": "ID", "required": True, "min": 2, "max": 3,
              "codes": ["1A", "BB", "EA"]},
        "2": {"name": "value", "type": "AN", "required": True, "min": 1, "max": 50},
    }},
    "DTM": {"elements": {"1": {"name": "qualifier", "type": "ID", "required": True,
                               "min": 3, "max": 3, "codes": ["232", "472"]}}},
}

# What the map gives 2100: 4010 positions scaled to 5010 numbering, a
# qualifier for the two REF places, and DTM01 narrower than its global list
# (the union with the DTM of 2110).
OCC_2100 = {
    "claim": {"segment": "CLP", "pos": 200, "usage": "required", "max": 1},
    "claim_date": {"segment": "DTM", "pos": 300, "usage": "situational", "max": 2,
                   "codes": {"1": ["232"]}},
    "other_id": {"segment": "REF", "pos": 400, "usage": "situational", "max": 5,
                 "qualifier": {"element": 1, "codes": ["BB", "EA"]}},
    "provider_id": {"segment": "REF", "pos": 400, "usage": "situational",
                    "qualifier": {"element": 1, "codes": ["1A"]}},
}


def occ_spec(tiny, loops_2100):
    spec = {
        "name": "occ",
        "loops": {
            "interchange": {"trigger": {"segment": "ISA"}, "end": "IEA", "occurrences": {
                "interchange_header": {"segment": "ISA", "pos": 100, "usage": "required",
                                       "max": 1}}},
            "2100": {"parent": "interchange", "trigger": {"segment": "CLP"}, **loops_2100},
            "2110": {"parent": "2100", "trigger": {"segment": "SVC"}},
        },
        "segments": OCC_SEGMENTS,
    }
    tiny["map.xml"].write_text(OCC_MAP)
    tiny["spec"].write_text(json.dumps(spec))


def test_occurrences_are_generated_from_the_map(tiny, tmp_path):
    occ_spec(tiny, {})
    script = load_script()
    report, patch = tmp_path / "report.md", tmp_path / "patch.json"
    assert script.main(args(tiny, "--report", str(report), "--patch", str(patch))) == 0
    draft = json.loads(patch.read_text())
    assert draft["loops"]["2100"] == {"max": 2, "occurrences": OCC_2100}
    assert "interchange" not in draft["loops"], "the spec's interchange already agrees"
    text = report.read_text()
    assert ("- 2100: `claim` CLP@200 R max 1; `claim_date` DTM@300 S max 2; `other_id` "
            "REF@400 ['BB', 'EA'] S max 5; `provider_id` REF@400 ['1A'] S") in text
    # A mapped loop that declares no occurrences fails the check.
    assert script.main(args(tiny, "--check")) == 1


def test_check_fails_on_a_loop_without_occurrences(tiny, capsys):
    occ_spec(tiny, {})
    assert load_script().main(args(tiny, "--check")) == 1
    err = capsys.readouterr().err
    assert ("occurrences:2100: loop 2100 declares no occurrences; pyx12 places 4 in it"
            in err)
    assert ("occurrences:2110: loop 2110 declares no occurrences; pyx12 places 2 in it"
            in err)
    assert "2 disagreement(s)" in err


def test_check_matches_occurrences_by_segment_and_qualifier_not_by_name(tiny, capsys):
    occurrences = dict(OCC_2100)
    occurrences["claim_header"] = occurrences.pop("claim")
    occurrences["other_id"] = {**occurrences["other_id"], "max": 4}
    occurrences["extra"] = {"segment": "AMT", "pos": 500}
    occ_spec(tiny, {"max": 2, "occurrences": occurrences})
    assert load_script().main(args(tiny, "--check")) == 1
    err = capsys.readouterr().err
    assert ("occurrences:2100:other_id:max: loop 2100 occurrence other_id: max is 4 "
            "in the spec, 5 in pyx12") in err
    assert ("occurrences:2100:extra: loop 2100: spec occurrence extra has no "
            "counterpart in pyx12") in err
    assert "claim_header" not in err, "a renamed occurrence still matches its place"
    # The extra AMT is also a segment pyx12 does not place in the loop.
    assert "segments:2100:AMT: loop 2100 holds AMT in the spec" in err
    # 2110 declares no occurrences.
    assert "4 disagreement(s)" in err


def test_check_names_an_occurrence_the_spec_lacks_and_a_loop_max(tiny, capsys):
    occurrences = dict(OCC_2100)
    del occurrences["provider_id"]
    occurrences["other_id"] = {**occurrences["other_id"],
                               "qualifier": {"element": 1, "codes": ["1A", "BB", "EA"]}}
    occ_spec(tiny, {"occurrences": occurrences})
    assert load_script().main(args(tiny, "--check")) == 1
    err = capsys.readouterr().err
    assert "occurrences:2100:max: loop 2100: max is None in the spec, 2 in pyx12" in err
    # Both REF places meet the spec's single REF, so neither is matched.
    assert ("loop 2100: pyx12 occurrence provider_id (REF 'Provider Id') has no counterpart "
            "in the spec") in err


def test_positions_number_both_versions_alike():
    script = load_script()
    assert script._position("030") == script._position("0300") == 300
    assert script._position("0100", table=3) == 30100
    assert script._position(None) == 0


def test_loops_nested_in_a_wrapper_table_share_its_position_space(tmp_path):
    path = tmp_path / "map.xml"
    path.write_text("""<?xml version="1.0"?>
<transaction xid="T">
  <loop xid="ST_LOOP" type="explicit"><usage>R</usage>
    <segment xid="ST"><pos>010</pos></segment>
    <loop xid="HEADER" type="wrapper">
      <segment xid="BPR"><pos>020</pos></segment>
      <loop xid="1000A"><usage>R</usage><segment xid="N1"><pos>080</pos></segment></loop>
    </loop>
    <loop xid="DETAIL" type="wrapper">
      <loop xid="2000"><usage>S</usage><segment xid="LX"><pos>003</pos></segment>
        <loop xid="2100"><segment xid="CLP"><pos>010</pos></segment></loop>
      </loop>
    </loop>
  </loop>
</transaction>
""")
    loops = {loop.xid: loop for loop in load_script().load_map(path)}
    first = {xid: loop.segments[0].pos for xid, loop in loops.items() if loop.segments}
    assert first == {"ST_LOOP": 100, "HEADER": 10200, "1000A": 10800, "2000": 20030,
                     "2100": 20100}
    assert (loops["ST_LOOP"].usage, loops["2000"].usage, loops["2100"].usage) == ("R", "S", "")


def test_merge_diff_is_the_patch_between_two_objects():
    script = load_script()
    base = {"a": {"x": 1, "y": [1]}, "b": 2}
    target = {"a": {"x": 1, "y": [2]}, "c": 3}
    diff = script.merge_diff(base, target)
    assert diff == {"a": {"y": [2]}, "b": None, "c": 3}
    assert script.merge_patch(base, diff) == target
