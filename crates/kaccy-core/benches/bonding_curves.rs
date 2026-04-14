use criterion::{Criterion, criterion_group, criterion_main};
use kaccy_core::pricing::{
    AdaptiveBondingCurve, BancorBondingCurve, BondingCurve, ExponentialBondingCurve,
    LinearBondingCurve, LogarithmicBondingCurve, SigmoidBondingCurve, SquareRootBondingCurve,
};
use rust_decimal_macros::dec;
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Linear
// ---------------------------------------------------------------------------

fn bench_linear(c: &mut Criterion) {
    let curve = LinearBondingCurve::new(dec!(0.0001), dec!(0.00001));

    let mut g = c.benchmark_group("linear");

    g.bench_function("spot_price/supply=1000", |b| {
        b.iter(|| black_box(curve.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("buy_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.buy_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sell_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.sell_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("linear/sweep", |b| {
        b.iter(|| {
            let mut supply = dec!(100);
            while supply <= dec!(10000) {
                black_box(curve.buy_price(black_box(supply), black_box(dec!(5))));
                supply += dec!(100);
            }
        })
    });

    g.finish();
}

// ---------------------------------------------------------------------------
// Bancor
// ---------------------------------------------------------------------------

fn bench_bancor(c: &mut Criterion) {
    let curve = BancorBondingCurve::new(dec!(1.0), dec!(0.3), dec!(1000));

    let mut g = c.benchmark_group("bancor");

    g.bench_function("spot_price/supply=1000", |b| {
        b.iter(|| black_box(curve.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("buy_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.buy_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sell_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.sell_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("bancor/sweep", |b| {
        b.iter(|| {
            let mut supply = dec!(100);
            while supply <= dec!(10000) {
                black_box(curve.buy_price(black_box(supply), black_box(dec!(5))));
                supply += dec!(100);
            }
        })
    });

    g.finish();
}

// ---------------------------------------------------------------------------
// Exponential
// ---------------------------------------------------------------------------

fn bench_exponential(c: &mut Criterion) {
    let curve = ExponentialBondingCurve::new(dec!(0.0001), dec!(0.001));

    let mut g = c.benchmark_group("exponential");

    g.bench_function("spot_price/supply=1000", |b| {
        b.iter(|| black_box(curve.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("buy_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.buy_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sell_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.sell_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("exponential/sweep", |b| {
        b.iter(|| {
            let mut supply = dec!(100);
            while supply <= dec!(10000) {
                black_box(curve.buy_price(black_box(supply), black_box(dec!(5))));
                supply += dec!(100);
            }
        })
    });

    g.finish();
}

// ---------------------------------------------------------------------------
// Sigmoid
// ---------------------------------------------------------------------------

fn bench_sigmoid(c: &mut Criterion) {
    let curve = SigmoidBondingCurve::new(dec!(0.01), dec!(0.005), dec!(5000));

    let mut g = c.benchmark_group("sigmoid");

    g.bench_function("spot_price/supply=1000", |b| {
        b.iter(|| black_box(curve.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("buy_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.buy_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sell_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.sell_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sigmoid/sweep", |b| {
        b.iter(|| {
            let mut supply = dec!(100);
            while supply <= dec!(10000) {
                black_box(curve.buy_price(black_box(supply), black_box(dec!(5))));
                supply += dec!(100);
            }
        })
    });

    g.finish();
}

// ---------------------------------------------------------------------------
// Adaptive
// ---------------------------------------------------------------------------

fn bench_adaptive(c: &mut Criterion) {
    let curve = AdaptiveBondingCurve::with_defaults(dec!(0.0001), dec!(0.01), dec!(100000));

    let mut g = c.benchmark_group("adaptive");

    g.bench_function("spot_price/supply=1000", |b| {
        b.iter(|| black_box(curve.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("buy_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.buy_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sell_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.sell_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("adaptive/sweep", |b| {
        b.iter(|| {
            let mut supply = dec!(100);
            while supply <= dec!(10000) {
                black_box(curve.buy_price(black_box(supply), black_box(dec!(5))));
                supply += dec!(100);
            }
        })
    });

    g.finish();
}

// ---------------------------------------------------------------------------
// Square root
// ---------------------------------------------------------------------------

fn bench_sqrt(c: &mut Criterion) {
    let curve = SquareRootBondingCurve::new(dec!(0.0001), dec!(1000));

    let mut g = c.benchmark_group("sqrt");

    g.bench_function("spot_price/supply=1000", |b| {
        b.iter(|| black_box(curve.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("buy_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.buy_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sell_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.sell_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sqrt/sweep", |b| {
        b.iter(|| {
            let mut supply = dec!(100);
            while supply <= dec!(10000) {
                black_box(curve.buy_price(black_box(supply), black_box(dec!(5))));
                supply += dec!(100);
            }
        })
    });

    g.finish();
}

// ---------------------------------------------------------------------------
// Logarithmic
// ---------------------------------------------------------------------------

fn bench_logarithmic(c: &mut Criterion) {
    let curve = LogarithmicBondingCurve::new_natural_log(dec!(0.0001), dec!(1000));

    let mut g = c.benchmark_group("logarithmic");

    g.bench_function("spot_price/supply=1000", |b| {
        b.iter(|| black_box(curve.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("buy_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.buy_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("sell_price/supply=1000,amount=10", |b| {
        b.iter(|| black_box(curve.sell_price(black_box(dec!(1000)), black_box(dec!(10)))))
    });

    g.bench_function("logarithmic/sweep", |b| {
        b.iter(|| {
            let mut supply = dec!(100);
            while supply <= dec!(10000) {
                black_box(curve.buy_price(black_box(supply), black_box(dec!(5))));
                supply += dec!(100);
            }
        })
    });

    g.finish();
}

// ---------------------------------------------------------------------------
// Comparison: all 7 curves spot_price(1000) side-by-side
// ---------------------------------------------------------------------------

fn bench_all_spot(c: &mut Criterion) {
    let linear = LinearBondingCurve::new(dec!(0.0001), dec!(0.00001));
    let bancor = BancorBondingCurve::new(dec!(1.0), dec!(0.3), dec!(1000));
    let exponential = ExponentialBondingCurve::new(dec!(0.0001), dec!(0.001));
    let sigmoid = SigmoidBondingCurve::new(dec!(0.01), dec!(0.005), dec!(5000));
    let adaptive = AdaptiveBondingCurve::with_defaults(dec!(0.0001), dec!(0.01), dec!(100000));
    let sqrt = SquareRootBondingCurve::new(dec!(0.0001), dec!(1000));
    let logarithmic = LogarithmicBondingCurve::new_natural_log(dec!(0.0001), dec!(1000));

    let mut g = c.benchmark_group("all_spot/supply=1000");

    g.bench_function("linear", |b| {
        b.iter(|| black_box(linear.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("bancor", |b| {
        b.iter(|| black_box(bancor.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("exponential", |b| {
        b.iter(|| black_box(exponential.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("sigmoid", |b| {
        b.iter(|| black_box(sigmoid.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("adaptive", |b| {
        b.iter(|| black_box(adaptive.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("sqrt", |b| {
        b.iter(|| black_box(sqrt.spot_price(black_box(dec!(1000)))))
    });

    g.bench_function("logarithmic", |b| {
        b.iter(|| black_box(logarithmic.spot_price(black_box(dec!(1000)))))
    });

    g.finish();
}

criterion_group!(
    benches,
    bench_linear,
    bench_bancor,
    bench_exponential,
    bench_sigmoid,
    bench_adaptive,
    bench_sqrt,
    bench_logarithmic,
    bench_all_spot,
);
criterion_main!(benches);
