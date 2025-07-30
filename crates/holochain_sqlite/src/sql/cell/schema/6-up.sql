-- All the content in this table will be rebuilt by Kitsune2 if it is missing.
-- Because we are fixing a logic issue with how ops are located in the DHT, clearing this table is the simplest way to
-- ensure that when nodes upgrade, they will rebuild the SliceHash table with the correct data.
DELETE FROM
  SliceHash;
