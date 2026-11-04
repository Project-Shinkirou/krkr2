#include "ncbind.hpp"
#include <set>
#include <exception>

// static変数の実体

// auto register 先頭ポインタ
ncbAutoRegister::ThisClassT const*
ncbAutoRegister::_top[ncbAutoRegister::LINE_COUNT] = NCB_INNER_AUTOREGISTER_LINES_INSTANCE;

std::map<ttstr, ncbAutoRegister::INTERNAL_PLUGIN_LISTS > ncbAutoRegister::_internal_plugins;
std::vector<const ncbAutoRegister *> ncbAutoRegister::_sessionRegistrars;

void ncbAutoRegister::EndSession() {
    std::exception_ptr failure;
    for(auto it = _sessionRegistrars.rbegin(); it != _sessionRegistrars.rend(); ++it) {
        try { (*it)->Unregist(); } catch(...) {
            if(!failure) failure = std::current_exception();
        }
    }
    _sessionRegistrars.clear();
    TVPRegisteredPlugins.clear();
    _internal_plugins.clear();
    if(failure) std::rethrow_exception(failure);
}

bool ncbAutoRegister::LoadModule(const ttstr &_name)
{
	ttstr name = _name.AsLowerCase();
	// already load
    if (TVPRegisteredPlugins.find(name) != TVPRegisteredPlugins.end())
		return true;
	auto it = _internal_plugins.find(name);
	if (it != _internal_plugins.end()) {
		for (const auto & plugin_list : it->second.lists) {
            for (auto i : plugin_list) {
                _sessionRegistrars.push_back(i);
				i->Regist();
			}
		}
		TVPRegisteredPlugins.insert(name);
		return true;
	}
	return false;
}
