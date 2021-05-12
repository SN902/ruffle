use crate::avm1::activation::Activation;
use crate::avm1::error::Error;
use crate::avm1::object::Object;
use crate::avm1::property_decl::{define_properties_on, Declaration};
use crate::avm1::{AvmString, ScriptObject, Value};
use crate::avm_warn;
use gc_arena::MutationContext;
use std::convert::Into;

const OBJECT_DECLS: &[Declaration] = declare_properties! {
    "PolicyFileResolver" => method(policy_file_resolver);
    "allowDomain" => method(allow_domain);
    "allowInsecureDomain" => method(allow_insecure_domain);
    "loadPolicyFile" => method(load_policy_file);
    "escapeDomain" => method(escape_domain);
    "sandboxType" => property(get_sandbox_type);
    "chooseLocalSwfPath" => property(get_choose_local_swf_path);
};

fn allow_domain<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm_warn!(activation, "System.security.allowDomain() not implemented");
    Ok(Value::Undefined)
}

fn allow_insecure_domain<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm_warn!(
        activation,
        "System.security.allowInsecureDomain() not implemented"
    );
    Ok(Value::Undefined)
}

fn load_policy_file<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm_warn!(
        activation,
        "System.security.allowInsecureDomain() not implemented"
    );
    Ok(Value::Undefined)
}

fn escape_domain<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm_warn!(activation, "System.security.escapeDomain() not implemented");
    Ok(Value::Undefined)
}

fn get_sandbox_type<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    Ok(AvmString::new(
        activation.context.gc_context,
        activation.context.system.sandbox_type.to_string(),
    )
    .into())
}

fn get_choose_local_swf_path<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm_warn!(
        activation,
        "System.security.chooseLocalSwfPath() not implemented"
    );
    Ok(Value::Undefined)
}

fn policy_file_resolver<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm_warn!(
        activation,
        "System.security.chooseLocalSwfPath() not implemented"
    );
    Ok(Value::Undefined)
}

pub fn create<'gc>(
    gc_context: MutationContext<'gc, '_>,
    proto: Option<Object<'gc>>,
    fn_proto: Object<'gc>,
) -> Object<'gc> {
    let security = ScriptObject::object(gc_context, proto);
    define_properties_on(OBJECT_DECLS, gc_context, security, fn_proto);
    security.into()
}
