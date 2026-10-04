from importlib import metadata


def test_the_base_package_requires_nothing():
    requirements = metadata.requires("oxedi835") or []
    assert requirements and all("extra ==" in r for r in requirements)


def test_the_extras_are_the_documented_ones():
    assert sorted(metadata.metadata("oxedi835").get_all("Provides-Extra")) == [
        "edi-835-parser", "pandas", "polars", "test",
    ]
