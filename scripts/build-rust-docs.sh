#!/bin/bash

cd target/doc
git init
echo '<meta http-equiv="refresh" content="0; url=https://dutterbutter.github.io/forest/forest_vm/index.html">' > index.html
git add .
git config --global -l
git -c user.name='dutterbutter' -c user.email='ci' commit -m 'Deploy documentation'
echo "${GITHUB_TOKEN}"
git push -f -q https://git:${GITHUB_TOKEN}@github.com/dutterbutter/forest HEAD:gh-pages