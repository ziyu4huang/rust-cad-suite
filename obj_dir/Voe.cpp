// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Model implementation (design independent parts)

#include "Voe__pch.h"

//============================================================
// Constructors

Voe::Voe(VerilatedContext* _vcontextp__, const char* _vcname__)
    : VerilatedModel{*_vcontextp__}
    , vlSymsp{new Voe__Syms(contextp(), _vcname__, this)}
    , rootp{&(vlSymsp->TOP)}
{
    // Register model with the context
    contextp()->addModel(this);
}

Voe::Voe(const char* _vcname__)
    : Voe(Verilated::threadContextp(), _vcname__)
{
}

//============================================================
// Destructor

Voe::~Voe() {
    delete vlSymsp;
}

//============================================================
// Evaluation function

#ifdef VL_DEBUG
void Voe___024root___eval_debug_assertions(Voe___024root* vlSelf);
#endif  // VL_DEBUG
void Voe___024root___eval_static(Voe___024root* vlSelf);
void Voe___024root___eval_initial(Voe___024root* vlSelf);
void Voe___024root___eval_settle(Voe___024root* vlSelf);
void Voe___024root___eval(Voe___024root* vlSelf);

void Voe::eval_step() {
    VL_DEBUG_IF(VL_DBG_MSGF("+++++TOP Evaluate Voe::eval_step\n"); );
#ifdef VL_DEBUG
    // Debug assertions
    Voe___024root___eval_debug_assertions(&(vlSymsp->TOP));
#endif  // VL_DEBUG
    vlSymsp->__Vm_deleter.deleteAll();
    if (VL_UNLIKELY(!vlSymsp->__Vm_didInit)) {
        VL_DEBUG_IF(VL_DBG_MSGF("+ Initial\n"););
        Voe___024root___eval_static(&(vlSymsp->TOP));
        Voe___024root___eval_initial(&(vlSymsp->TOP));
        Voe___024root___eval_settle(&(vlSymsp->TOP));
        vlSymsp->__Vm_didInit = true;
    }
    VL_DEBUG_IF(VL_DBG_MSGF("+ Eval\n"););
    Voe___024root___eval(&(vlSymsp->TOP));
    // Evaluate cleanup
    Verilated::endOfEval(vlSymsp->__Vm_evalMsgQp);
}

//============================================================
// Events and timing
bool Voe::eventsPending() { return !vlSymsp->TOP.__VdlySched.empty() && !contextp()->gotFinish(); }

uint64_t Voe::nextTimeSlot() { return vlSymsp->TOP.__VdlySched.nextTimeSlot(); }

//============================================================
// Utilities

const char* Voe::name() const {
    return vlSymsp->name();
}

//============================================================
// Invoke final blocks

void Voe___024root___eval_final(Voe___024root* vlSelf);

VL_ATTR_COLD void Voe::final() {
    contextp()->executingFinal(true);
    Voe___024root___eval_final(&(vlSymsp->TOP));
    contextp()->executingFinal(false);
}

//============================================================
// Implementations of abstract methods from VerilatedModel

const char* Voe::hierName() const { return vlSymsp->name(); }
const char* Voe::modelName() const { return "Voe"; }
unsigned Voe::threads() const { return 1; }
void Voe::prepareClone() const { contextp()->prepareClone(); }
void Voe::atClone() const {
    contextp()->threadPoolpOnClone();
}
