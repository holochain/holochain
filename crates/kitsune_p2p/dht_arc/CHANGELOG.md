# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

- Slight modifications to the arc resizing algorithm to improve stability.
  - Instead of aiming for the ideal target arc size, aim for an ideal range. This slack in the system allows all agents to converge on their target instead of endlessly oscillating.
  - No longer take "gap detection" into consideration, as it was seen to have a negative effect on stability.
## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
