---
title: "Artificial Intelligence, Algorithmic Pricing, and Collusion"
authors:
  - Emilio Calvano
  - Giacomo Calzolari
  - Vincenzo Denicolò
  - Sergio Pastorello
year: 2020
journal: American Economic Review
citekey: calvano2020aicollusion
---

## Summary

This paper develops a framework to assess the economic impact of algorithmic recommender systems (RSs), conceptualizing them as creating personalized prominence in consumer search. The authors integrate realistic collaborative-filtering algorithms trained on synthetic data with a flexible model of consumer preferences and product differentiation. They apply this framework to analyze three key issues: how algorithmic recommendations influence market concentration and diversity of consumer choices, their effects on equilibrium prices and consumer welfare, and the potential for platforms to manipulate recommendations to prioritize more profitable products (self-preferencing).

## Key Contributions

- Develops a novel framework that conceptualizes recommender systems as creating personalized prominence in consumer search, rather than assuming consumers mechanically follow recommendations
- Integrates latent-factor collaborative-filtering algorithms with an address model of product differentiation that mirrors the algorithm's structure, allowing controlled analysis of RS impacts
- Demonstrates that RSs tend to favor mass-market products over niche ones due to a "uniformity effect" where algorithms overestimate consumer similarity
- Shows that RSs lead firms to raise prices, and that the relationship between algorithmic information and consumer welfare follows an inverted-U curve
- Finds that platform manipulation of recommendations causes prices of over-recommended products to decrease, mitigating negative welfare impacts

## Methodology

The paper employs a numerical approach combining state-of-the-art collaborative-filtering algorithms with a structural model of consumer preferences and search. The authors implement latent-factor matrix factorization algorithms (similar to the Netflix Prize winner) and train them on synthetic data generated from a Lancastrian model of product differentiation. Consumer preferences and product characteristics are represented as vectors in a latent-factor space, where utility is the inner product of consumer taste vectors and product attribute vectors. The model accommodates both horizontal and vertical differentiation through a parameterization that varies product locations from a circle (pure horizontal) to a square (pure vertical).

The framework embeds algorithmic recommendations within a fully developed search model. Consumers receive personalized recommendations that provide pre-search information, allowing them to start searching with products likely to match their preferences while maintaining flexibility to continue searching if suggested products are poor matches. The authors numerically determine optimal consumer search patterns and firm pricing strategies, comparing scenarios with personalized recommendations against benchmarks with unassisted random search. Extensive robustness analysis addresses external validity concerns.

## Main Results

- RSs exhibit a "uniformity" bias that favors mass-market products over niche alternatives, as algorithms overestimate consumer similarity and recommend products aligned with median consumer preferences; this bias only disappears with very high data quantity and quality
- RSs lead to higher equilibrium prices even without price discrimination, though they also improve consumer-product matching and reduce search costs; consumer surplus may decline in markets with few products, predominantly horizontal differentiation, and high search costs
- The relationship between algorithmic information and consumer welfare follows an inverted-U curve: initially, better information helps consumers find suitable products, but beyond a threshold, information-driven price increases dominate
- When platforms manipulate recommendations (self-preferencing), prices of over-recommended products tend to decrease, limiting both the welfare harm and profitability of such manipulation
- Data endogeneity creates a feedback loop where algorithms gather better information on previously recommended items, creating bias that reduces recommendation quality compared to randomly generated training data

## Related Work

- [[lee2021]] - Lee and Wright (2021): Assess information value of RS algorithms by comparing to random choices
- [[castellini2023]] - Castellini et al. (2023): Use complete information as benchmark for RS analysis
- [[leemusolff2023]] - Lee and Musolff (2023): Empirically uncover price effects of algorithmic recommendations
- [[johnson2023]] - Johnson et al. (2023): Algorithmic collusion in industrial organization
- [[asker2023]] - Asker et al. (2023): AI and algorithmic collusion
- [[klein2021]] - Klein (2021): Algorithmic collusion analysis
- [[wolinsky1986]] - Wolinsky (1986): Classic search model foundation
- [[andersonrenault1999]] - Anderson and Renault (1999): Search model with product differentiation