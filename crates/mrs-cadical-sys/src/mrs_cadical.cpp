#include "cadical.hpp"
#include "ccadical.h"
#include "tracer.hpp"

#include <cstddef>
#include <cstdint>

extern "C" {

struct MrsCaDiCaL {
  CaDiCaL::Solver *solver;
  void *tracer;
};

struct TraceCallbacks {
  void *userdata;
  void (*begin_proof)(void *, int64_t);
  void (*add_original_clause)(void *, int64_t, int, const int *, size_t, int);
  void (*add_derived_clause)(void *, int64_t, int, int, const int *, size_t,
                             const int64_t *, size_t);
  void (*delete_clause)(void *, int64_t, int, const int *, size_t);
  void (*demote_clause)(void *, int64_t, const int *, size_t);
  void (*weaken_minus)(void *, int64_t, const int *, size_t);
  void (*strengthen)(void *, int64_t);
  void (*finalize_clause)(void *, int64_t, const int *, size_t);
  void (*report_status)(void *, int, int64_t);
  void (*solve_query)(void *);
  void (*add_assumption)(void *, int);
  void (*add_constraint)(void *, const int *, size_t);
  void (*reset_assumptions)(void *);
  void (*add_assumption_clause)(void *, int64_t, const int *, size_t,
                                const int64_t *, size_t);
  void (*conclude_unsat)(void *, int, const int64_t *, size_t);
  void (*conclude_sat)(void *, const int *, size_t);
  void (*conclude_unknown)(void *, const int *, size_t);
  void (*notify_equivalence)(void *, int, int);
};

}

namespace {

struct CallbackTracer final : CaDiCaL::Tracer {
  TraceCallbacks callbacks;

  explicit CallbackTracer(const TraceCallbacks &callbacks) : callbacks(callbacks) {}

  void begin_proof(int64_t id) override {
    if (callbacks.begin_proof)
      callbacks.begin_proof(callbacks.userdata, id);
  }

  void add_original_clause(int64_t id, bool redundant,
                           const std::vector<int> &clause,
                           bool restored) override {
    if (callbacks.add_original_clause)
      callbacks.add_original_clause(callbacks.userdata, id, redundant,
                                    clause.data(), clause.size(), restored);
  }

  void add_derived_clause(int64_t id, bool redundant, int witness,
                          const std::vector<int> &clause,
                          const std::vector<int64_t> &antecedents) override {
    if (callbacks.add_derived_clause)
      callbacks.add_derived_clause(callbacks.userdata, id, redundant, witness,
                                   clause.data(), clause.size(),
                                   antecedents.data(), antecedents.size());
  }

  void delete_clause(int64_t id, bool redundant,
                     const std::vector<int> &clause) override {
    if (callbacks.delete_clause)
      callbacks.delete_clause(callbacks.userdata, id, redundant, clause.data(),
                              clause.size());
  }

  void demote_clause(uint64_t id, const std::vector<int> &clause) override {
    if (callbacks.demote_clause)
      callbacks.demote_clause(callbacks.userdata, static_cast<int64_t>(id),
                              clause.data(), clause.size());
  }

  void weaken_minus(int64_t id, const std::vector<int> &clause) override {
    if (callbacks.weaken_minus)
      callbacks.weaken_minus(callbacks.userdata, id, clause.data(), clause.size());
  }

  void strengthen(int64_t id) override {
    if (callbacks.strengthen)
      callbacks.strengthen(callbacks.userdata, id);
  }

  void finalize_clause(int64_t id, const std::vector<int> &clause) override {
    if (callbacks.finalize_clause)
      callbacks.finalize_clause(callbacks.userdata, id, clause.data(),
                                clause.size());
  }

  void report_status(int status, int64_t id) override {
    if (callbacks.report_status)
      callbacks.report_status(callbacks.userdata, status, id);
  }

  void solve_query() override {
    if (callbacks.solve_query)
      callbacks.solve_query(callbacks.userdata);
  }

  void add_assumption(int lit) override {
    if (callbacks.add_assumption)
      callbacks.add_assumption(callbacks.userdata, lit);
  }

  void add_constraint(const std::vector<int> &clause) override {
    if (callbacks.add_constraint)
      callbacks.add_constraint(callbacks.userdata, clause.data(), clause.size());
  }

  void reset_assumptions() override {
    if (callbacks.reset_assumptions)
      callbacks.reset_assumptions(callbacks.userdata);
  }

  void add_assumption_clause(int64_t id, const std::vector<int> &clause,
                             const std::vector<int64_t> &antecedents) override {
    if (callbacks.add_assumption_clause)
      callbacks.add_assumption_clause(callbacks.userdata, id, clause.data(),
                                      clause.size(), antecedents.data(),
                                      antecedents.size());
  }

  void conclude_unsat(CaDiCaL::ConclusionType conclusion,
                      const std::vector<int64_t> &ids) override {
    if (callbacks.conclude_unsat)
      callbacks.conclude_unsat(callbacks.userdata, static_cast<int>(conclusion),
                               ids.data(), ids.size());
  }

  void conclude_sat(const std::vector<int> &model) override {
    if (callbacks.conclude_sat)
      callbacks.conclude_sat(callbacks.userdata, model.data(), model.size());
  }

  void conclude_unknown(const std::vector<int> &trail) override {
    if (callbacks.conclude_unknown)
      callbacks.conclude_unknown(callbacks.userdata, trail.data(), trail.size());
  }

  void notify_equivalence(int first, int second) override {
    if (callbacks.notify_equivalence)
      callbacks.notify_equivalence(callbacks.userdata, first, second);
  }
};

bool valid(const MrsCaDiCaL *wrapper) { return wrapper && wrapper->solver; }

} // namespace

extern "C" {

const char *mrs_cadical_version() { return CaDiCaL::Solver::version(); }

MrsCaDiCaL *mrs_cadical_init() {
  auto *wrapper = new MrsCaDiCaL{};
  wrapper->solver = new CaDiCaL::Solver();
  wrapper->tracer = nullptr;
  return wrapper;
}

void mrs_cadical_release(MrsCaDiCaL *wrapper) {
  if (!wrapper)
    return;
  if (wrapper->tracer) {
    auto *tracer = static_cast<CallbackTracer *>(wrapper->tracer);
    wrapper->solver->disconnect_proof_tracer(
        static_cast<CaDiCaL::Tracer *>(tracer));
    delete tracer;
  }
  delete wrapper->solver;
  delete wrapper;
}

void mrs_cadical_add_clause(MrsCaDiCaL *wrapper, const int *clause, size_t len) {
  if (!valid(wrapper))
    return;
  for (size_t i = 0; i < len; ++i)
    wrapper->solver->add(clause[i]);
  wrapper->solver->add(0);
}

void mrs_cadical_assume(MrsCaDiCaL *wrapper, int lit) {
  if (valid(wrapper))
    wrapper->solver->assume(lit);
}

void mrs_cadical_add_constraint(MrsCaDiCaL *wrapper, const int *clause,
                                size_t len) {
  if (!valid(wrapper))
    return;
  for (size_t i = 0; i < len; ++i)
    wrapper->solver->constrain(clause[i]);
  wrapper->solver->constrain(0);
}

int mrs_cadical_solve(MrsCaDiCaL *wrapper) {
  return valid(wrapper) ? wrapper->solver->solve() : 0;
}

int mrs_cadical_status(const MrsCaDiCaL *wrapper) {
  return valid(wrapper) ? wrapper->solver->status() : 0;
}

int mrs_cadical_value(const MrsCaDiCaL *wrapper, int lit) {
  return valid(wrapper) ? wrapper->solver->val(lit) : 0;
}

int mrs_cadical_failed(const MrsCaDiCaL *wrapper, int lit) {
  return valid(wrapper) && wrapper->solver->failed(lit);
}

int mrs_cadical_vars(const MrsCaDiCaL *wrapper) {
  return valid(wrapper) ? wrapper->solver->vars() : 0;
}

int mrs_cadical_declare_more_variables(MrsCaDiCaL *wrapper, int count) {
  return valid(wrapper) ? wrapper->solver->declare_more_variables(count) : 0;
}

int mrs_cadical_declare_one_more_variable(MrsCaDiCaL *wrapper) {
  return valid(wrapper) ? wrapper->solver->declare_one_more_variable() : 0;
}

int mrs_cadical_set_option(MrsCaDiCaL *wrapper, const char *name, int value) {
  return valid(wrapper) && name && wrapper->solver->set(name, value);
}

int mrs_cadical_get_option(const MrsCaDiCaL *wrapper, const char *name) {
  return valid(wrapper) && name ? wrapper->solver->get(name) : 0;
}

int mrs_cadical_set_limit(MrsCaDiCaL *wrapper, const char *name, int value) {
  return valid(wrapper) && name && wrapper->solver->limit(name, value);
}

int mrs_cadical_configure(MrsCaDiCaL *wrapper, const char *name) {
  return valid(wrapper) && name && wrapper->solver->configure(name);
}

void mrs_cadical_phase(MrsCaDiCaL *wrapper, int lit) {
  if (valid(wrapper))
    wrapper->solver->phase(lit);
}

void mrs_cadical_unphase(MrsCaDiCaL *wrapper, int lit) {
  if (valid(wrapper))
    wrapper->solver->unphase(lit);
}

void mrs_cadical_freeze(MrsCaDiCaL *wrapper, int lit) {
  if (valid(wrapper))
    wrapper->solver->freeze(lit);
}

void mrs_cadical_melt(MrsCaDiCaL *wrapper, int lit) {
  if (valid(wrapper))
    wrapper->solver->melt(lit);
}

int mrs_cadical_frozen(const MrsCaDiCaL *wrapper, int lit) {
  return valid(wrapper) && wrapper->solver->frozen(lit);
}

void mrs_cadical_terminate(MrsCaDiCaL *wrapper) {
  if (valid(wrapper))
    wrapper->solver->terminate();
}

int mrs_cadical_trace_proof(MrsCaDiCaL *wrapper, const char *path, int format,
                            int binary) {
  if (!valid(wrapper) || !path)
    return 0;
  if (format == 1) {
    wrapper->solver->set("lrat", 1);
    wrapper->solver->set("frat", 0);
  } else if (format == 2 || format == 3) {
    wrapper->solver->set("lrat", 0);
    wrapper->solver->set("frat", format == 2 ? 1 : 2);
  } else {
    wrapper->solver->set("lrat", 0);
    wrapper->solver->set("frat", 0);
  }
  wrapper->solver->set("binary", binary != 0);
  return wrapper->solver->trace_proof(path);
}

void mrs_cadical_flush_proof(MrsCaDiCaL *wrapper) {
  if (valid(wrapper))
    wrapper->solver->flush_proof_trace();
}

void mrs_cadical_close_proof(MrsCaDiCaL *wrapper) {
  if (valid(wrapper))
    wrapper->solver->close_proof_trace();
}

int mrs_cadical_connect_trace(MrsCaDiCaL *wrapper,
                              const TraceCallbacks *callbacks, int antecedents,
                              int finalize_clauses) {
  if (!valid(wrapper) || !callbacks || wrapper->tracer)
    return 0;
  auto *tracer = new CallbackTracer(*callbacks);
  wrapper->tracer = tracer;
  wrapper->solver->connect_proof_tracer(
      static_cast<CaDiCaL::Tracer *>(tracer), antecedents != 0,
      finalize_clauses != 0);
  return 1;
}

int mrs_cadical_disconnect_trace(MrsCaDiCaL *wrapper) {
  if (!valid(wrapper) || !wrapper->tracer)
    return 0;
  auto *tracer = static_cast<CallbackTracer *>(wrapper->tracer);
  const bool disconnected = wrapper->solver->disconnect_proof_tracer(
      static_cast<CaDiCaL::Tracer *>(tracer));
  delete tracer;
  wrapper->tracer = nullptr;
  return disconnected;
}

}
