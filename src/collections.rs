//! Ordered collection primitives ported from src/utils/set.ts and
//! src/utils/objectGroupBy.ts. Key identity is supplied by Rust Eq/Hash.
//! Set insertion order is preserved to match JavaScript Set iteration order.
use indexmap::{IndexMap,IndexSet};
use std::hash::Hash;

pub fn difference<T:Eq+Hash+Clone>(a:&IndexSet<T>,b:&IndexSet<T>)->IndexSet<T>{
    a.iter().filter(|item|!b.contains(*item)).cloned().collect()
}

pub fn intersects<T:Eq+Hash>(a:&IndexSet<T>,b:&IndexSet<T>)->bool{
    if a.is_empty()||b.is_empty(){return false}
    a.iter().any(|item|b.contains(item))
}

pub fn every<T:Eq+Hash>(a:&IndexSet<T>,b:&IndexSet<T>)->bool{
    a.iter().all(|item|b.contains(item))
}

pub fn union<T:Eq+Hash+Clone>(a:&IndexSet<T>,b:&IndexSet<T>)->IndexSet<T>{
    a.iter().chain(b.iter()).cloned().collect()
}

/// Group by a selector invoked exactly once per item with the original
/// zero-based index. Keys such as "__proto__" are plain data, not prototype
/// mutations, matching the JS source's Object.create(null).
pub fn object_group_by<T,K,I,F>(items:I,mut selector:F)->IndexMap<K,Vec<T>>
where
    I:IntoIterator<Item=T>,
    K:Eq+Hash,
    F:FnMut(&T,usize)->K
{
    let mut groups=IndexMap::new();
    for (index,item) in items.into_iter().enumerate(){
        let key=selector(&item,index);
        groups.entry(key).or_insert_with(Vec::new).push(item);
    }
    groups
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn ordered_set_functions_match_original_empty_subset_union_and_intersection(){
        let empty=IndexSet::<i32>::new();
        let a=IndexSet::from_iter([3,1,2]);
        let b=IndexSet::from_iter([2,4,3]);
        assert_eq!(difference(&a,&b).into_iter().collect::<Vec<_>>(),vec![1]);
        assert_eq!(union(&a,&b).into_iter().collect::<Vec<_>>(),vec![3,1,2,4]);
        assert_eq!(difference(&a,&empty).into_iter().collect::<Vec<_>>(),vec![3,1,2]);
        assert_eq!(union(&empty,&b).into_iter().collect::<Vec<_>>(),vec![2,4,3]);
        assert!(intersects(&a,&b));
        assert!(!intersects(&a,&empty));
        assert!(!intersects(&empty,&a));
        assert!(every(&empty,&a));
        assert!(every(&a,&a));
        assert!(every(&IndexSet::from_iter([3,2]),&a));
        assert!(!every(&b,&a));
    }

    #[test]
    fn group_by_keeps_zero_based_callback_indices_and_original_item_order(){
        let mut seen=Vec::new();
        let groups=object_group_by(["a","b","c","d"],|item,index|{
            seen.push((item.to_string(),index));
            if index%2==0{"even"}else{"odd"}
        });
        assert_eq!(seen,vec![("a".into(),0),("b".into(),1),("c".into(),2),("d".into(),3)]);
        assert_eq!(groups["even"],vec!["a","c"]);
        assert_eq!(groups["odd"],vec!["b","d"]);
        assert_eq!(groups.keys().copied().collect::<Vec<_>>(),vec!["even","odd"]);
        let empty=object_group_by(Vec::<i32>::new(),|_,_|String::from("never"));
        assert!(empty.is_empty());
    }

    #[test]
    fn null_prototype_source_keys_are_not_special_or_dropped(){
        let grouped=object_group_by(["__proto__","constructor","toString","__proto__"],
            |item,_|(*item).to_string());
        assert_eq!(grouped["__proto__"],vec!["__proto__","__proto__"]);
        assert_eq!(grouped["constructor"],vec!["constructor"]);
        assert_eq!(grouped["toString"],vec!["toString"]);
        assert_eq!(grouped.len(),3);
    }
}
