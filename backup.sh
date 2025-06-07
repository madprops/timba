#!/usr/bin/env bash
git add -A
git commit -m "Nothing"
git checkout -b backup3
git add -A
git commit -m "Backup on $(date +%Y-%m-%d)"
git push origin backup3
git checkout main
git branch -D backup3
