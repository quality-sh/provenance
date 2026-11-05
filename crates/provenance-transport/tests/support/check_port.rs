use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckPort, GapsContext, PortFuture,
    Refusal,
};

pub struct FixtureCheckPort;

impl CheckPort for FixtureCheckPort {
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async move {
            Ok(match category {
                Category::Graph => CategoryRun::Graph {
                    findings: Vec::new(),
                    refusal: Refusal::None,
                },
                Category::Statements => CategoryRun::Statements {
                    findings: Vec::new(),
                    context: None,
                    refusal: Refusal::None,
                },
                Category::Bindings => CategoryRun::Bindings {
                    findings: Vec::new(),
                    context: BindingContext {
                        policy: BindingPolicy::Warning,
                    },
                    refusal: Refusal::None,
                },
                Category::Gaps => CategoryRun::Gaps {
                    findings: Vec::new(),
                    context: GapsContext { scopes: Vec::new() },
                },
            })
        })
    }
}
