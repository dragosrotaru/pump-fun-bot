-- add the virtual_sol_liquidity and virtual_token_liquidity columns to the price table
ALTER TABLE prices ADD COLUMN virtual_sol_liquidity float8;
ALTER TABLE prices ADD COLUMN virtual_token_liquidity float8;
-- move the sol_liquidity and token_liquidity columns to real_sol_liquidity and real_token_liquidity
ALTER TABLE prices RENAME COLUMN sol_liquidity TO real_sol_liquidity;
ALTER TABLE prices RENAME COLUMN token_liquidity TO real_token_liquidity;
