# install psydk
cd ../crates/psydk
pip install -e .

# run sphinx
sphinx-build -M html docs/source/ docs/build/ -W -a -j auto -n --keep-going"
