# Setheum Elastic Reserve Protocol - SERP
SERP-TES Pallet -- SERP-Token Elasticity of Supply

## Overview

 The SERP-TES (Setheum Elastic Reserve Protocol - Token Elasticity of Supply) 
 module provides a token elasticity system for the SERP-STP258 mixed stablecoin system, 
 by configuring an expansion which implements a `supply_change` to calculate supply_change
 and an `on_serp_block` which determines if it is time to Serp / adjust supply or not.

 Then to determine whether the SERP should expand or contract supply, the TES provides
 a `serp_elast` to tell the TES when to expand and when to contract supply depending on 
 the outcome of the price of the stablecoin / settcurrency.

 The serp-tes module provides functionality of both the `Stp258` module that needs 
 to contract and expand the supply of its currencies for its stablecoin stability  system through the `SerpTes`
 and the `SerpMarket` module that needs to serp-trade the currencies expanded and 
 contracted by the `SerpTes` module, which it has to do with the `SerpStaking` module to be 
 built in the next Milestone of the Serp Modules.
 