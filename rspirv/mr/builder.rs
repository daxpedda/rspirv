// Copyright 2017 Google Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(dead_code)]

use mr;
use spirv;

use std::result;
use super::Error;

type BuildResult<T> = result::Result<T, Error>;

/// The memory representation builder.
///
/// Constructs a [`Module`](struct.Module.html) by aggregating results from
/// method calls for various instructions. Most of the methods return the
/// SPIR-V id assigned for that SPIR-V instruction.
pub struct Builder {
    module: mr::Module,
    next_id: u32,
    function: Option<mr::Function>,
    basic_block: Option<mr::BasicBlock>,
}

impl Builder {
    /// Creates a new empty builder.
    pub fn new() -> Builder {
        Builder {
            module: mr::Module::new(),
            next_id: 1,
            function: None,
            basic_block: None,
        }
    }

    /// Returns the `Module` under construction.
    pub fn module(self) -> mr::Module {
        self.module
    }

    #[inline(always)]
    fn id(&mut self) -> spirv::Word {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Begins building of a new function.
    pub fn begin_function(&mut self,
                          return_type: spirv::Word,
                          control: spirv::FunctionControl,
                          function_type: spirv::Word)
                          -> BuildResult<spirv::Word> {
        if self.function.is_some() {
            return Err(Error::NestedFunction);
        }

        let id = self.id();

        let mut f = mr::Function::new();
        f.def = Some(mr::Instruction::new(spirv::Op::Function,
                                          Some(return_type),
                                          Some(id),
                                          vec![mr::Operand::FunctionControl(control),
                                               mr::Operand::IdRef(function_type)]));
        self.function = Some(f);
        Ok(id)
    }

    /// Ends building of the current function.
    pub fn end_function(&mut self) -> BuildResult<()> {
        if self.function.is_none() {
            return Err(Error::MismatchedFunctionEnd);
        }

        let mut f = self.function.take().unwrap();
        f.end = Some(mr::Instruction::new(spirv::Op::FunctionEnd, None, None, vec![]));
        Ok(self.module.functions.push(f))
    }

    /// Begins building of a new basic block.
    pub fn begin_basic_block(&mut self) -> BuildResult<spirv::Word> {
        if self.function.is_none() {
            return Err(Error::DetachedBasicBlock);
        }
        if self.basic_block.is_some() {
            return Err(Error::NestedBasicBlock);
        }

        let id = self.id();

        let mut bb = mr::BasicBlock::new();
        bb.label = Some(mr::Instruction::new(spirv::Op::Label, None, None, vec![]));

        self.basic_block = Some(bb);
        Ok(id)
    }

    fn end_basic_block(&mut self, inst: mr::Instruction) -> BuildResult<()> {
        if self.basic_block.is_none() {
            return Err(Error::MismatchedTerminator);
        }

        self.basic_block.as_mut().unwrap().instructions.push(inst);
        Ok(self.function.as_mut().unwrap().basic_blocks.push(self.basic_block.take().unwrap()))
    }

    pub fn capability(&mut self, capability: spirv::Capability) {
        let inst = mr::Instruction::new(spirv::Op::Capability,
                                        None,
                                        None,
                                        vec![mr::Operand::Capability(capability)]);
        self.module.capabilities.push(inst);
    }

    pub fn extension(&mut self, extension: String) {
        let inst = mr::Instruction::new(spirv::Op::Extension,
                                        None,
                                        None,
                                        vec![mr::Operand::LiteralString(extension)]);
        self.module.extensions.push(inst);
    }

    pub fn ext_inst_import(&mut self, extended_inst_set: String) -> spirv::Word {
        let id = self.id();
        let inst = mr::Instruction::new(spirv::Op::ExtInstImport,
                                        None,
                                        Some(id),
                                        vec![mr::Operand::LiteralString(extended_inst_set)]);
        self.module.ext_inst_imports.push(inst);
        id
    }

    pub fn memory_model(&mut self,
                        addressing_model: spirv::AddressingModel,
                        memory_model: spirv::MemoryModel) {
        let inst = mr::Instruction::new(spirv::Op::MemoryModel,
                                        None,
                                        None,
                                        vec![mr::Operand::AddressingModel(addressing_model),
                                             mr::Operand::MemoryModel(memory_model)]);
        self.module.memory_model = Some(inst);
    }

    pub fn entry_point(&mut self,
                       execution_model: spirv::ExecutionModel,
                       entry_point: spirv::Word,
                       name: String,
                       interface: &[spirv::Word]) {
        let mut operands = vec![mr::Operand::ExecutionModel(execution_model),
                                mr::Operand::IdRef(entry_point),
                                mr::Operand::LiteralString(name)];
        for v in interface {
            operands.push(mr::Operand::IdRef(*v));
        }

        let inst = mr::Instruction::new(spirv::Op::EntryPoint, None, None, operands);
        self.module.entry_points.push(inst);
    }

    pub fn execution_mode(&mut self,
                          entry_point: spirv::Word,
                          execution_mode: spirv::ExecutionMode,
                          params: &[u32]) {
        let mut operands = vec![mr::Operand::IdRef(entry_point),
                                mr::Operand::ExecutionMode(execution_mode)];
        for v in params {
            operands.push(mr::Operand::LiteralInt32(*v));
        }

        let inst = mr::Instruction::new(spirv::Op::ExecutionMode, None, None, operands);
        self.module.execution_modes.push(inst);
    }
}

include!("build_type.rs");
include!("build_terminator.rs");

impl Builder {
    /// Creates an OpDecorate instruction and returns the result id.
    pub fn decorate(&mut self,
                    target: spirv::Word,
                    decoration: spirv::Decoration,
                    mut params: Vec<mr::Operand>)
                    -> spirv::Word {
        let id = self.id();
        let mut operands = vec![mr::Operand::IdRef(target), mr::Operand::Decoration(decoration)];
        operands.append(&mut params);
        self.module
            .annotations
            .push(mr::Instruction::new(spirv::Op::Decorate, None, Some(id), operands));
        id
    }

    /// Creates an OpMemberDecorate instruction and returns the result id.
    pub fn member_decorate(&mut self,
                           structure: spirv::Word,
                           member: spirv::Word,
                           decoration: spirv::Decoration,
                           mut params: Vec<mr::Operand>)
                           -> spirv::Word {
        let id = self.id();
        let mut operands = vec![mr::Operand::IdRef(structure),
                                mr::Operand::IdRef(member),
                                mr::Operand::Decoration(decoration)];
        operands.append(&mut params);
        self.module
            .annotations
            .push(mr::Instruction::new(spirv::Op::MemberDecorate, None, Some(id), operands));
        id
    }

    /// Creates an OpDecorationGroup instruction and returns the result id.
    pub fn decoration_group(&mut self) -> spirv::Word {
        let id = self.id();
        self.module
            .annotations
            .push(mr::Instruction::new(spirv::Op::DecorationGroup, None, Some(id), vec![]));
        id
    }

    /// Creates an OpGroupDecorate instruction and returns the result id.
    pub fn group_decorate(&mut self, group: spirv::Word, targets: Vec<spirv::Word>) -> spirv::Word {
        let id = self.id();
        let mut operands = vec![mr::Operand::IdRef(group)];
        for v in targets {
            operands.push(mr::Operand::IdRef(v));
        }
        self.module
            .annotations
            .push(mr::Instruction::new(spirv::Op::GroupDecorate, None, Some(id), operands));
        id
    }

    /// Creates an OpGroupMemberDecorate instruction and returns the result id.
    pub fn group_member_decorate(&mut self,
                                 group: spirv::Word,
                                 targets: Vec<(spirv::Word, u32)>)
                                 -> spirv::Word {
        let id = self.id();
        let mut operands = vec![mr::Operand::IdRef(group)];
        for (target, member) in targets {
            operands.push(mr::Operand::IdRef(target));
            operands.push(mr::Operand::LiteralInt32(member));
        }
        self.module
            .annotations
            .push(mr::Instruction::new(spirv::Op::GroupMemberDecorate, None, Some(id), operands));
        id
    }
}
