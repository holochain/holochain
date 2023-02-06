-- consider existing block from *s to *e
-- __ __ *s __ __ *e __ __
--
-- rather than look at all overlapping possibilites
-- there are only two possibilities for !overlapping
-- :e is strictly less than *s
-- :s :e
-- :s is strictly greater than *e
-- __ __ __ __ __ __ :s :e
--
-- overlapping = !!overlapping
-- => !(:e < *s || *e < :s)
--
-- this is true IFF the caller ensures that :s <= :e
-- i.e. the caller MUST provide a valid span to compare for calculating overlap.
FROM
  BlockSpan
WHERE
  target_id = :target_id
  AND target_reason = :target_reason
  AND NOT(
    :end_ms < start_ms
    OR end_ms < :start_ms
  )
