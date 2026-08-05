//! 报销标准配置（多套标准集）集成测试
//! 单文件只有一个 #[test]，保证进程内 static 串行、不互相干扰。
//! 结束时 apply_config(&ReimbursementConfig::default()) 复位全局状态。

use invoice_reimbursement_lib::models::hotel_standard::{
    get_builtin_hotel_standards, get_hotel_nightly_rate_std,
};
use invoice_reimbursement_lib::models::reimbursement_config::{
    apply_config, CityStandard, ProvinceStandard, ReimbursementConfig, StandardSet,
};
use invoice_reimbursement_lib::pdf::build_reimbursement_form;

#[test]
fn test_standard_sets_process_wide() {
    // 先复位，避免本地残留配置影响断言
    apply_config(&ReimbursementConfig::default());

    // 1. 构造用户集：福建省 380（其他城市），福州 500（城市覆盖），集兜底 400
    let mut cfg = ReimbursementConfig::default();
    cfg.meal_subsidy_daily = 120.0;
    let set = StandardSet {
        id: "set-a".to_string(),
        name: "我的标准".to_string(),
        default_hotel_standard: 400.0,
        provinces: vec![ProvinceStandard {
            name: "福建省".to_string(),
            default_standard: 380.0,
            cities: vec![CityStandard {
                name: "福州".to_string(),
                standard: 500.0,
            }],
        }],
    };
    cfg.active_standard_set_id = set.id.clone();
    cfg.standard_sets.push(set);
    apply_config(&cfg);

    // 2. 激活集匹配：城市覆盖 / 省其他城市 / 集兜底
    assert_eq!(get_hotel_nightly_rate_std("福州市"), 500.0);
    assert_eq!(get_hotel_nightly_rate_std("泉州市"), 380.0);
    assert_eq!(get_hotel_nightly_rate_std("未知地点"), 400.0);

    // 3. 伙食补助：2025-01-01 → 2025-01-06 = 6 天 × 120 = 720
    let form = build_reimbursement_form(
        &[], "", "", "", "2025-01-01", "2025-01-06", 0, "其他人员",
    );
    assert_eq!(form.travel_days, 6);
    assert_eq!(form.meal_subsidy.daily_rate, 120.0);
    assert_eq!(form.meal_subsidy.amount, 720.0);

    // 4. 复位到 builtin（active="builtin" → 无激活用户集，走内置逻辑）
    apply_config(&ReimbursementConfig::default());
    assert_eq!(get_hotel_nightly_rate_std("深圳市"), 450.0);
    assert_eq!(get_hotel_nightly_rate_std("成都市"), 370.0);

    // 5. 内置结构派生：福建省 default=380 且厦门市挂在福建省下；直辖市独立节点
    let builtin = get_builtin_hotel_standards();
    let fujian = builtin
        .iter()
        .find(|p| p.name == "福建省")
        .expect("福建省节点应存在");
    assert_eq!(fujian.default_standard, 380.0);
    assert!(fujian
        .cities
        .iter()
        .any(|c| c.name == "厦门市" && c.standard == 400.0));
    assert!(builtin.iter().any(|p| p.name == "北京市"));

    // 复位，避免影响其他测试
    apply_config(&ReimbursementConfig::default());
}
