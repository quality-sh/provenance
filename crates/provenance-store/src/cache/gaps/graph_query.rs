use super::graph_index::GraphIndex;
use super::record_ref::RecordRef;
use provenance_core::model::relations::RecordFront;
use provenance_core::{
    Boundary, Contribution, Domain, NodeType, Question, Requirement, Resolution, Rule, ScopeId,
    Source, StableId, SynthesisPacket, Thread, Topic,
};

pub struct GapGraph<'a> {
    pub scope: &'a ScopeId,
    pub sources: &'a [Source],
    pub requirements: &'a [Requirement],
    pub resolutions: &'a [Resolution],
    pub rules: &'a [Rule],
    pub topics: &'a [Topic],
    pub questions: &'a [Question],
    pub threads: &'a [Thread],
    pub domains: &'a [Domain],
    pub boundaries: &'a [Boundary],
    /// Ideation records are not graph nodes; they are here for the one
    /// gap their `target` can open.
    pub contributions: &'a [Contribution],
    pub synthesis_packets: &'a [SynthesisPacket],
}

impl GapGraph<'_> {
    /// The traversal front over these records.
    pub const fn front(&self) -> RecordFront<'_> {
        RecordFront {
            sources: self.sources,
            requirements: self.requirements,
            resolutions: self.resolutions,
            rules: self.rules,
            topics: self.topics,
            questions: self.questions,
            domains: self.domains,
            boundaries: self.boundaries,
        }
    }
}

/// Typed lookups and graph joins over a [`GapGraph`].
///
/// Gap policy is written against these helpers, and they are the single
/// home for the traversals the wiki assembler needs too, so both readers
/// answer "what resolves this?" and "what did this produce?" the same
/// way. Constructing the query builds one index over the graph; every
/// lookup then answers from that index in the record order of the
/// underlying vectors, and never rescans them per query.
pub struct GraphQuery<'a, 'graph> {
    pub graph: &'a GapGraph<'graph>,
    pub(super) index: GraphIndex<'graph>,
}

impl<'a, 'graph> GraphQuery<'a, 'graph> {
    pub fn new(graph: &'a GapGraph<'graph>) -> Self {
        Self {
            graph,
            index: GraphIndex::new(graph),
        }
    }

    /// The first record holding the id under the node kind, in record
    /// order. Ids are kind-qualified: a Requirement and a Resolution
    /// sharing an id are two distinct records.
    pub fn find(&self, node_type: NodeType, id: &str) -> Option<RecordRef<'graph>> {
        let position = *self.index.records_with_id(node_type, id).first()?;
        Some(self.record_at(node_type, position))
    }

    /// Every record holding the id under the node kind, in record order.
    /// Duplicate records with one id stay distinct rows.
    pub fn all(&self, node_type: NodeType, id: &str) -> Vec<RecordRef<'graph>> {
        self.index
            .records_with_id(node_type, id)
            .iter()
            .map(|position| self.record_at(node_type, *position))
            .collect()
    }

    fn record_at(&self, node_type: NodeType, position: usize) -> RecordRef<'graph> {
        match node_type {
            NodeType::Source => RecordRef::Source(&self.graph.sources[position]),
            NodeType::Requirement => RecordRef::Requirement(&self.graph.requirements[position]),
            NodeType::Resolution => RecordRef::Resolution(&self.graph.resolutions[position]),
            NodeType::Rule => RecordRef::Rule(&self.graph.rules[position]),
            NodeType::Topic => RecordRef::Topic(&self.graph.topics[position]),
            NodeType::Question => RecordRef::Question(&self.graph.questions[position]),
            NodeType::Domain => RecordRef::Domain(&self.graph.domains[position]),
            NodeType::Boundary => RecordRef::Boundary(&self.graph.boundaries[position]),
        }
    }

    /// The requirement with this id, or none when the scope holds no
    /// such requirement.
    pub fn find_requirement(&self, id: &StableId) -> Option<&'graph Requirement> {
        self.index
            .records_with_id(NodeType::Requirement, id.as_str())
            .first()
            .map(|position| &self.graph.requirements[*position])
    }

    /// The source with this id, or none when the scope holds no such
    /// source.
    pub fn find_source(&self, id: &StableId) -> Option<&'graph Source> {
        self.index
            .records_with_id(NodeType::Source, id.as_str())
            .first()
            .map(|position| &self.graph.sources[*position])
    }

    pub fn source_exists(&self, id: &StableId) -> bool {
        self.find(NodeType::Source, id.as_str()).is_some()
    }

    pub fn requirement_exists(&self, id: &StableId) -> bool {
        self.find(NodeType::Requirement, id.as_str()).is_some()
    }

    pub fn resolution_exists(&self, id: &StableId) -> bool {
        self.find(NodeType::Resolution, id.as_str()).is_some()
    }

    pub fn topic_exists(&self, id: &StableId) -> bool {
        self.find(NodeType::Topic, id.as_str()).is_some()
    }

    pub fn node_exists(&self, node_type: NodeType, id: &StableId) -> bool {
        self.find(node_type, id.as_str()).is_some()
    }
}
