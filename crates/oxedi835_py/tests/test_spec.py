import json

import pytest

from oxedi835 import Spec, SpecError

PATCH = {"loops": {"ZZ": {"parent": "2100", "trigger": {"segment": "ZZ1"}}}}


def test_the_builtin_spec_loads_and_round_trips_through_json():
    spec = Spec.builtin()
    assert "2100" in spec.loops()
    assert Spec.from_json(spec.to_json()).to_json() == spec.to_json()


def test_a_bad_spec_raises_spec_error_with_the_core_message():
    with pytest.raises(SpecError) as info:
        Spec.from_json('{"name": "x", "loops": {"a": {"trigger": 1}}}')
    assert isinstance(info.value, ValueError)
    assert str(info.value) == (
        "spec: the value at loops.a.trigger must be a JSON object; found a number"
    )


def test_invalid_json_names_the_parser_message():
    with pytest.raises(SpecError) as info:
        Spec.from_json("{")
    assert str(info.value) == (
        "invalid spec JSON: EOF while parsing an object at line 1 column 1"
    )


@pytest.mark.parametrize("as_text", [False, True])
def test_a_patch_adds_a_loop(as_text):
    patch = json.dumps(PATCH) if as_text else PATCH
    patched = Spec.builtin().patch(patch)
    assert "ZZ" in patched.loops()
    assert "ZZ" not in Spec.builtin().loops()


def test_a_patch_that_breaks_the_spec_raises_with_the_inner_error():
    bad = {"loops": {"ZZ": {"parent": "nope", "trigger": {"segment": "ZZ1"}}}}
    with pytest.raises(SpecError) as info:
        Spec.builtin().patch(bad)
    assert str(info.value) == 'applying patch: loop "ZZ" names unknown parent "nope"'
    assert type(info.value).__module__ == "oxedi835"


def test_a_patch_must_be_a_dict_or_a_string():
    with pytest.raises(TypeError) as info:
        Spec.builtin().patch(3)
    assert str(info.value) == "Spec.patch takes a dict or a JSON string, not int"


def test_a_dict_that_json_cannot_write_raises_type_error():
    with pytest.raises(TypeError) as info:
        Spec.builtin().patch({"loops": {1}})
    assert str(info.value) == (
        "Spec.patch: the dict cannot be written as JSON: "
        "Object of type set is not JSON serializable"
    )


def test_the_spec_repr():
    assert repr(Spec.builtin()) == "Spec(name='835', loops=8, tables=5)"
