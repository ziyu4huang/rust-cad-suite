// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Symbol table internal header
//
// Internal details; most calling programs do not need this header,
// unless using verilator public meta comments.

#ifndef VERILATED_VOE__SYMS_H_
#define VERILATED_VOE__SYMS_H_  // guard

#include "verilated.h"

// INCLUDE MODEL CLASS

#include "Voe.h"

// INCLUDE MODULE CLASSES
#include "Voe___024root.h"

// SYMS CLASS (contains all model state)
class alignas(VL_CACHE_LINE_BYTES) Voe__Syms final : public VerilatedSyms {
  public:
    // INTERNAL STATE
    Voe* const __Vm_modelp;
    VlDeleter __Vm_deleter;
    bool __Vm_didInit = false;

    // MODULE INSTANCE STATE
    Voe___024root                  TOP;

    // CONSTRUCTORS
    Voe__Syms(VerilatedContext* contextp, const char* namep, Voe* modelp);
    ~Voe__Syms();

    // METHODS
    const char* name() const { return TOP.vlNamep; }
};

#endif  // guard
