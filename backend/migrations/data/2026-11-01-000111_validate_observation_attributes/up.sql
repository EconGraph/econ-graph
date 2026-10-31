-- A separate migration transaction releases the column-addition lock before scanning history.
ALTER TABLE data_points VALIDATE CONSTRAINT data_points_attributes_object;
