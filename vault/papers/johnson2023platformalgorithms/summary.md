---
title: "Platform Design When Sellers Use Pricing Algorithms"
authors:
  - Justin P. Johnson
  - Andrew Rhodes
  - Matthijs Wildenbeest
year: 2023
journal: Econometrica
citekey: johnson2023platformalgorithms
---

## Summary

This paper investigates how online platforms can design marketplace rules to promote competition, improve consumer surplus, and increase platform profits, particularly when sellers use AI pricing algorithms. The authors develop and analyze two demand-steering policies: Price-Directed Prominence (PDP), which rewards lower-priced sellers with additional consumer exposure, and Dynamic PDP, which also conditions on past pricing behavior. Using both theoretical analysis and simulations with Q-learning algorithms, they demonstrate that while simple PDP policies can benefit consumers when markets are competitive, more sophisticated dynamic policies that condition on past behavior are necessary to disrupt algorithmic collusion. The research shows that platform design can meaningfully benefit consumers even when AI algorithms would otherwise learn to collude and sustain supracompetitive prices.

## Key Contributions

- Develops theoretical framework for analyzing platform design rules that steer consumer demand based on seller pricing, showing PDP increases consumer surplus when no more than ~63% of products are obscured in competitive markets
- Demonstrates that Dynamic PDP can destabilize collusion even when firms are nearly infinitely patient, which is theoretically equivalent to algorithms operating in real-time
- Provides extensive Q-learning simulations showing algorithms typically learn to set prices above Bertrand-Nash levels, confirming concerns about algorithmic collusion
- Shows that algorithms respond to PDP by adopting price cycles to rotate demand and split profits, but Dynamic PDP disrupts this strategy, leading to substantially lower prices
- Establishes that simple PDP policies perform well relative to arbitrarily sophisticated policies despite requiring minimal information (no knowledge of costs or demand needed)

## Methodology

The authors employ a dual approach combining economic theory with AI simulations. The theoretical model features n firms selling differentiated products on a monopoly platform using a standard logit demand framework, where firms interact repeatedly over an infinite horizon. The platform earns per-unit commissions and may weight consumer utility in its objective function. Two design policies are analyzed: PDP displays only the k lowest-priced firms, while Dynamic PDP extends additional demand to price-cutting firms in subsequent periods subject to maintaining low prices.

For the simulation component, the authors use Q-learning algorithms, a reinforcement-learning approach popular in computer science that learns optimal strategies based on historical experience with the environment. The simulations assess how algorithms respond to platform design rules across various discount factors and product differentiation levels, examining both price levels and pricing patterns (constant prices versus price cycles). This approach captures the growing real-world use of AI in automated repricing software on platforms like Amazon.

## Main Results

- In competitive markets, PDP drives prices to effective marginal cost and benefits consumers and platforms when k/n exceeds approximately 0.37 (i.e., at least 37% of products are shown)
- Under full collusion, PDP alone performs poorly and can reduce consumer surplus regardless of how many products are obscured, as cartels can sustain elevated prices when discount factors are high
- Dynamic PDP causes dramatic price drops in simulations even with high discount factors, generating large increases in consumer surplus and moderate increases in platform commissions
- Algorithms without platform intervention learn to set constant prices that split industry profits nearly equally; under PDP they adopt price cycles to rotate demand, but Dynamic PDP disrupts this profit-splitting mechanism
- The critical discount factor for sustaining collusion increases as fewer products are shown under PDP, making collusion harder to maintain

## Related Work

- [[calvano2020artificial]] - Foundational simulation work showing Q-learning algorithms learn collusive strategies with reward-and-punishment schemes in differentiated product markets
- [[klein2021autonomous]] - Studies algorithmic strategies with homogeneous products and alternating price changes, finding Edgeworth cycles supporting supranormal profits
- [[brown2023competition]] - Shows non-AI algorithms enable price commitment that raises overall prices
- [[dinerstein2018consumer]] - Empirical study of platform design recognizing tradeoffs between prices and variety
- [[harrington2018developing]] - Discusses policy issues around algorithmic collusion and whether collusion requires explicit agreement
- [[ezrachi2016virtual]] - Raises concerns about algorithms facilitating collusion through various mechanisms including AI learning